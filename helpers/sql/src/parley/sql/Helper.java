// Parley's SQL helper (R-SQL-11, D20): runs SQL on an IBM i through JTOpen's JDBC driver for
// Parley's SQL window. Parley starts it with `java -cp parley-sql.jar:jt400.jar parley.sql.Helper`
// and talks to it in JSON, one object per line: requests on stdin, answers on stdout. It ends
// when stdin closes, so it never outlives Parley. The password arrives only on stdin and never
// reaches stdout, stderr or an error. MIT OR Apache-2.0, as the rest of Parley; JTOpen is IBM's,
// under the IBM Public License 1.0 (helpers/sql/NOTICE.txt).
//
// Requests:  {"id":1,"op":"hello"}
//            {"id":2,"op":"connect","host":"h","tls":false,"user":"u","password":"p","ca_file":""}
//            {"id":3,"op":"run","sql":"SELECT ...","max_rows":1000}
//            {"id":4,"op":"fetch","cursor":1,"max_rows":1000}   {"id":5,"op":"close_cursor","cursor":1}
//            {"id":6,"op":"autocommit","on":false}   {"id":7,"op":"commit"}   {"id":8,"op":"rollback"}
//            {"op":"cancel"} (no answer; stops the statement running)   {"id":9,"op":"close"}
// Answers:   {"id":n,"ok":{...}}  or  {"id":n,"error":{"message":"...","sqlstate":"...","sqlcode":-204}}
package parley.sql;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.io.PrintStream;
import java.math.BigDecimal;
import java.nio.charset.StandardCharsets;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.ResultSet;
import java.sql.ResultSetMetaData;
import java.sql.SQLException;
import java.sql.SQLTimeoutException;
import java.sql.Statement;
import java.sql.Types;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Properties;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.LinkedBlockingQueue;

public final class Helper {
    /** The protocol's version; Parley checks it in the answer to hello. */
    static final int PROTOCOL = 1;

    /** Seconds to give up connecting (R-SQL-3); a property so tests can shorten it. */
    private static final int LOGIN_TIMEOUT = Integer.getInteger("parley.sql.loginTimeout", 30);

    /** Rows fetched from the host at a time (R-SQL-4). */
    private static final int BLOCK = 500;

    private final PrintStream out;
    private Connection connection;
    private final Map<Long, Cursor> cursors = new HashMap<>();
    private long nextCursor = 1;
    /** The statement running now, for cancel; set and read across the two threads. */
    private volatile Statement running;

    private static final class Cursor {
        final Statement statement;
        final ResultSet rows;

        Cursor(Statement statement, ResultSet rows) {
            this.statement = statement;
            this.rows = rows;
        }
    }

    private Helper(PrintStream out) {
        this.out = out;
    }

    public static void main(String[] args) throws Exception {
        if (args.length > 0 && args[0].equals("--self-test")) {
            System.exit(SelfTest.run());
        }
        PrintStream out = new PrintStream(System.out, true, "UTF-8");
        Helper helper = new Helper(out);
        BlockingQueue<Map<String, Object>> work = new LinkedBlockingQueue<>();
        Thread worker = new Thread(() -> helper.work(work), "parley-sql-worker");
        worker.setDaemon(true);
        worker.start();
        BufferedReader in =
                new BufferedReader(new InputStreamReader(System.in, StandardCharsets.UTF_8));
        String line;
        boolean closing = false;
        while ((line = in.readLine()) != null) {
            if (line.trim().isEmpty()) {
                continue;
            }
            Map<String, Object> request;
            try {
                request = asMap(Json.parse(line));
            } catch (IllegalArgumentException e) {
                helper.answer(null, error(e.getMessage(), null, 0));
                continue;
            }
            if ("cancel".equals(request.get("op"))) {
                helper.cancel();
                continue;
            }
            work.put(request);
            if ("close".equals(request.get("op"))) {
                closing = true;
                break;
            }
        }
        if (closing) {
            // Asked to close: every request before it is answered first.
            worker.join();
        } else {
            // Input ended without a close: Parley is gone. Stop what runs and leave.
            helper.cancel();
            work.put(new HashMap<>());
            worker.join(5_000);
        }
        System.exit(0);
    }

    /** Runs the requests in order on the worker thread. */
    private void work(BlockingQueue<Map<String, Object>> work) {
        while (true) {
            Map<String, Object> request;
            try {
                request = work.take();
            } catch (InterruptedException e) {
                return;
            }
            Object op = request.get("op");
            if (op == null) {
                closeAll();
                return;
            }
            Object id = request.get("id");
            try {
                answer(id, ok(handle(String.valueOf(op), request)));
            } catch (SQLException e) {
                answer(id, error(e.getMessage(), e.getSQLState(), e.getErrorCode()));
            } catch (RuntimeException e) {
                answer(id, error(String.valueOf(e.getMessage()), null, 0));
            }
            if ("close".equals(op)) {
                closeAll();
                return;
            }
        }
    }

    private Object handle(String op, Map<String, Object> request) throws SQLException {
        switch (op) {
            case "hello":
                return hello();
            case "connect":
                return connect(request);
            case "run":
                return run(text(request, "sql"), maxRows(request), seconds(request, "timeout"));
            case "fetch":
                return fetch(number(request, "cursor"), maxRows(request));
            case "close_cursor":
                closeCursor(number(request, "cursor"));
                return new LinkedHashMap<>();
            case "autocommit":
                connected().setAutoCommit(Boolean.TRUE.equals(request.get("on")));
                return new LinkedHashMap<>();
            case "commit":
                connected().commit();
                return new LinkedHashMap<>();
            case "rollback":
                connected().rollback();
                return new LinkedHashMap<>();
            case "close":
                return new LinkedHashMap<>();
            default:
                throw new IllegalArgumentException("unknown request " + op);
        }
    }

    private Map<String, Object> hello() {
        Map<String, Object> about = new LinkedHashMap<>();
        about.put("protocol", (long) PROTOCOL);
        about.put("java", System.getProperty("java.version"));
        about.put("jtopen", jtopenVersion());
        return about;
    }

    private static String jtopenVersion() {
        try {
            Class<?> copyright = Class.forName("com.ibm.as400.access.Copyright");
            return String.valueOf(copyright.getField("version").get(null));
        } catch (ReflectiveOperationException | LinkageError e) {
            return "";
        }
    }

    private Map<String, Object> connect(Map<String, Object> request) throws SQLException {
        if (connection != null) {
            throw new IllegalArgumentException("already connected");
        }
        String host = text(request, "host");
        boolean tls = Boolean.TRUE.equals(request.get("tls"));
        Object caFile = request.get("ca_file");
        if (tls && caFile instanceof String && !((String) caFile).isEmpty()) {
            Trust.addCaFile((String) caFile);
        }
        Properties properties = new Properties();
        properties.setProperty("user", text(request, "user"));
        properties.setProperty("password", text(request, "password"));
        properties.setProperty("naming", "sql");
        properties.setProperty("date format", "iso");
        properties.setProperty("time format", "iso");
        properties.setProperty("prompt", "false");
        properties.setProperty("errors", "full");
        properties.setProperty("secure", tls ? "true" : "false");
        properties.setProperty("block size", "512");
        // The library and file each column comes from, for its heading and text (R-SQL-29).
        properties.setProperty("extended metadata", "true");
        // A query timeout cancels the statement after that long, rather than JTOpen's default
        // of letting the IBM i refuse a query by its estimated time (R-SQL-18).
        properties.setProperty("query timeout mechanism", "cancel");
        // Seconds to give up connecting; JTOpen's own default waits minutes for a silent host.
        properties.setProperty("login timeout", String.valueOf(LOGIN_TIMEOUT));
        request.remove("password");
        try {
            Class.forName("com.ibm.as400.access.AS400JDBCDriver");
        } catch (ClassNotFoundException e) {
            throw new IllegalArgumentException("jt400.jar is not on the class path");
        }
        connection = DriverManager.getConnection("jdbc:as400://" + host, properties);
        connection.setAutoCommit(true);
        Map<String, Object> about = new LinkedHashMap<>();
        about.put("job", serverJob(connection));
        about.put("release", connection.getMetaData().getDatabaseProductVersion());
        about.put("user", connection.getMetaData().getUserName());
        return about;
    }

    /** The database job, as number/user/name. */
    private static String serverJob(Connection connection) {
        try {
            Object id = connection.getClass().getMethod("getServerJobIdentifier").invoke(connection);
            String job = String.valueOf(id); // name (10), user (10), number (6)
            if (job.length() >= 26) {
                return job.substring(20, 26).trim() + "/" + job.substring(10, 20).trim() + "/"
                        + job.substring(0, 10).trim();
            }
            return job.trim();
        } catch (ReflectiveOperationException | RuntimeException e) {
            return "";
        }
    }

    private Map<String, Object> run(String sql, int maxRows, int timeout) throws SQLException {
        Statement statement = connected().createStatement();
        running = statement;
        boolean query;
        try {
            statement.setFetchSize(BLOCK);
            statement.setQueryTimeout(timeout); // 0: none (R-SQL-18)
            query = statement.execute(sql);
        } catch (SQLException e) {
            statement.close();
            if (timeout > 0 && (e instanceof SQLTimeoutException || "57014".equals(e.getSQLState()))) {
                throw new SQLException("The statement ran longer than " + timeout
                        + " seconds, so it was stopped (" + e.getMessage() + ")", e.getSQLState(),
                        e.getErrorCode(), e);
            }
            throw e;
        } finally {
            running = null;
        }
        Map<String, Object> result = new LinkedHashMap<>();
        if (!query) {
            result.put("kind", "count");
            result.put("count", (long) statement.getUpdateCount());
            statement.close();
            return result;
        }
        ResultSet rows = statement.getResultSet();
        long id = nextCursor++;
        cursors.put(id, new Cursor(statement, rows));
        result.put("kind", "rows");
        result.put("cursor", id);
        result.put("columns", columns(rows.getMetaData()));
        addHeadings(result);
        putRows(result, id, maxRows);
        return result;
    }

    private Map<String, Object> fetch(long id, int maxRows) throws SQLException {
        Map<String, Object> result = new LinkedHashMap<>();
        putRows(result, id, maxRows);
        return result;
    }

    /** Up to `maxRows` rows of cursor `id` as text, and whether there are more. */
    private void putRows(Map<String, Object> result, long id, int maxRows) throws SQLException {
        Cursor cursor = cursors.get(id);
        if (cursor == null) {
            throw new IllegalArgumentException("no cursor " + id);
        }
        ResultSetMetaData meta = cursor.rows.getMetaData();
        int count = meta.getColumnCount();
        List<Object> rows = new ArrayList<>();
        boolean more = true;
        running = cursor.statement;
        try {
            while (rows.size() < maxRows) {
                if (!cursor.rows.next()) {
                    more = false;
                    break;
                }
                List<Object> row = new ArrayList<>(count);
                for (int c = 1; c <= count; c++) {
                    row.add(Values.text(cursor.rows, c, meta.getColumnType(c)));
                }
                rows.add(row);
            }
        } finally {
            running = null;
        }
        result.put("rows", rows);
        result.put("more", more);
        if (!more) {
            closeCursor(id);
        }
    }

    private static List<Object> columns(ResultSetMetaData meta) throws SQLException {
        List<Object> columns = new ArrayList<>();
        for (int c = 1; c <= meta.getColumnCount(); c++) {
            Map<String, Object> column = new LinkedHashMap<>();
            // With extended metadata JTOpen puts the column heading in the label; the name is
            // the field's (or the alias), as the operator wrote it in the statement (R-SQL-29).
            String name = meta.getColumnName(c);
            String label = blankToNull(meta.getColumnLabel(c));
            column.put("name", name);
            column.put("field", blankToNull(name));
            column.put("label", label != null && !label.equals(name) ? heading(label) : null);
            column.put("schema", blankToNull(meta.getSchemaName(c)));
            column.put("table", blankToNull(meta.getTableName(c)));
            column.put("type", meta.getColumnTypeName(c));
            column.put("length", (long) meta.getPrecision(c));
            column.put("scale", (long) meta.getScale(c));
            column.put("nullable", meta.isNullable(c) != ResultSetMetaData.columnNoNulls);
            column.put("numeric", Values.isNumeric(meta.getColumnType(c)));
            columns.add(column);
        }
        return columns;
    }

    /** null for a missing or blank value, the value trimmed otherwise. */
    private static String blankToNull(String value) {
        return value == null || value.trim().isEmpty() ? null : value.trim();
    }

    /**
     * Each column's heading (COLHDG, LABEL ON: three parts of 20) and text from the catalog,
     * where the column comes from a file (R-SQL-29). A column the catalog does not answer for
     * keeps none: the lookup never fails the query.
     */
    @SuppressWarnings("unchecked")
    private void addHeadings(Map<String, Object> result) {
        String sql = "SELECT COLUMN_NAME, SYSTEM_COLUMN_NAME, COLUMN_HEADING, COLUMN_TEXT"
                + " FROM QSYS2.SYSCOLUMNS WHERE TABLE_SCHEMA = ?"
                + " AND (TABLE_NAME = ? OR SYSTEM_TABLE_NAME = ?)";
        Map<String, Map<String, String[]>> byTable = new HashMap<>();
        for (Object o : (List<Object>) result.get("columns")) {
            Map<String, Object> column = (Map<String, Object>) o;
            Object schema = column.get("schema");
            Object table = column.get("table");
            Object field = column.get("field");
            if (schema == null || table == null || field == null) {
                continue;
            }
            String key = schema + "/" + table;
            Map<String, String[]> catalog = byTable.computeIfAbsent(key, k -> catalog(sql,
                    (String) schema, (String) table));
            String[] found = catalog.get(((String) field).toUpperCase(java.util.Locale.ROOT));
            if (found != null) {
                column.put("heading", found[0]);
                column.put("text", found[1]);
            }
        }
        // Where the catalog had nothing (a view, an alias), the label JTOpen gave, if any.
        for (Object o : (List<Object>) result.get("columns")) {
            Map<String, Object> column = (Map<String, Object>) o;
            Object label = column.remove("label");
            if (column.get("heading") == null && label != null) {
                column.put("heading", label);
            }
        }
    }

    /** A file's columns from the catalog: name to {heading, text}; empty when not found. */
    private Map<String, String[]> catalog(String sql, String schema, String table) {
        Map<String, String[]> columns = new HashMap<>();
        try (java.sql.PreparedStatement query = connected().prepareStatement(sql)) {
            query.setString(1, schema);
            query.setString(2, table);
            query.setString(3, table);
            try (ResultSet rows = query.executeQuery()) {
                while (rows.next()) {
                    String[] about = {heading(rows.getString(3)), blankToNull(rows.getString(4))};
                    columns.put(rows.getString(1), about);
                    columns.put(rows.getString(2), about);
                }
            }
        } catch (SQLException e) {
            // No catalog answer (no authority, a temporary result): the columns keep their names.
        }
        return columns;
    }

    /**
     * A column heading on one line: its parts (up to three of 20, padded with blanks, or one
     * longer text) with each run of blanks made one space. Cutting at every 20 would split
     * words of a heading written as one text.
     */
    static String heading(String raw) {
        if (raw == null) {
            return null;
        }
        String joined = raw.trim().replaceAll("\\s+", " ");
        return joined.isEmpty() ? null : joined;
    }

    private void closeCursor(long id) {
        Cursor cursor = cursors.remove(id);
        if (cursor != null) {
            try {
                cursor.statement.close(); // closes its result set too
            } catch (SQLException e) {
                // A cursor the host already closed: nothing left to free.
            }
        }
    }

    private void cancel() {
        Statement statement = running;
        if (statement != null) {
            try {
                statement.cancel();
            } catch (SQLException e) {
                // Already finished: there is nothing to cancel.
            }
        }
    }

    private void closeAll() {
        for (Long id : new ArrayList<>(cursors.keySet())) {
            closeCursor(id);
        }
        if (connection != null) {
            try {
                connection.close();
            } catch (SQLException e) {
                // The connection is gone either way.
            }
            connection = null;
        }
    }

    private Connection connected() {
        if (connection == null) {
            throw new IllegalArgumentException("not connected");
        }
        return connection;
    }

    private synchronized void answer(Object id, Map<String, Object> body) {
        Map<String, Object> message = new LinkedHashMap<>();
        message.put("id", id);
        message.putAll(body);
        out.println(Json.write(message));
    }

    private static Map<String, Object> ok(Object value) {
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("ok", value);
        return body;
    }

    static Map<String, Object> error(String message, String sqlstate, int sqlcode) {
        Map<String, Object> detail = new LinkedHashMap<>();
        detail.put("message", message);
        detail.put("sqlstate", sqlstate);
        detail.put("sqlcode", (long) sqlcode);
        Map<String, Object> body = new LinkedHashMap<>();
        body.put("error", detail);
        return body;
    }

    @SuppressWarnings("unchecked")
    static Map<String, Object> asMap(Object value) {
        if (!(value instanceof Map)) {
            throw new IllegalArgumentException("a request must be a JSON object");
        }
        return (Map<String, Object>) value;
    }

    private static String text(Map<String, Object> request, String name) {
        Object value = request.get(name);
        if (!(value instanceof String)) {
            throw new IllegalArgumentException("the request needs \"" + name + "\" as text");
        }
        return (String) value;
    }

    private static long number(Map<String, Object> request, String name) {
        Object value = request.get(name);
        if (!(value instanceof Long)) {
            throw new IllegalArgumentException("the request needs \"" + name + "\" as a number");
        }
        return (Long) value;
    }

    /** A number of seconds in the request, 0 when absent; at most a day. */
    private static int seconds(Map<String, Object> request, String name) {
        Object value = request.get(name);
        long seconds = value instanceof Long ? (Long) value : 0;
        return (int) Math.max(0, Math.min(seconds, 86_400));
    }

    private static int maxRows(Map<String, Object> request) {
        Object value = request.get("max_rows");
        long rows = value instanceof Long ? (Long) value : 1000;
        return (int) Math.max(1, Math.min(rows, 100_000));
    }

    /** One column's value as text (R-SQL-4); null for NULL. */
    static final class Values {
        private Values() {}

        /** Seconds always shown, as on IBM i (Java's own text drops ":00"). */
        static final java.time.format.DateTimeFormatter TIME =
                java.time.format.DateTimeFormatter.ofPattern("HH:mm:ss");
        /** IBM i timestamps have microseconds. */
        static final java.time.format.DateTimeFormatter TIMESTAMP =
                java.time.format.DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm:ss.SSSSSS");

        static boolean isNumeric(int type) {
            switch (type) {
                case Types.DECIMAL:
                case Types.NUMERIC:
                case Types.SMALLINT:
                case Types.INTEGER:
                case Types.BIGINT:
                case Types.REAL:
                case Types.FLOAT:
                case Types.DOUBLE:
                case Types.TINYINT:
                    return true;
                default:
                    return false;
            }
        }

        static Object text(ResultSet rows, int c, int type) throws SQLException {
            switch (type) {
                case Types.DECIMAL:
                case Types.NUMERIC: {
                    BigDecimal value = rows.getBigDecimal(c);
                    return value == null ? null : value.toPlainString();
                }
                case Types.DATE: {
                    java.sql.Date value = rows.getDate(c);
                    return value == null ? null : value.toLocalDate().toString();
                }
                case Types.TIME: {
                    java.sql.Time value = rows.getTime(c);
                    return value == null ? null : Values.TIME.format(value.toLocalTime());
                }
                case Types.TIMESTAMP: {
                    java.sql.Timestamp value = rows.getTimestamp(c);
                    return value == null ? null : Values.TIMESTAMP.format(value.toLocalDateTime());
                }
                case Types.BINARY:
                case Types.VARBINARY:
                case Types.LONGVARBINARY:
                case Types.ROWID: {
                    byte[] value = rows.getBytes(c);
                    return value == null ? null : hex(value);
                }
                case Types.BLOB:
                case Types.CLOB:
                case Types.NCLOB:
                case Types.SQLXML: {
                    Object value = rows.getObject(c);
                    return value == null ? null : "(LOB)";
                }
                default: {
                    String value = rows.getString(c);
                    return rows.wasNull() ? null : value;
                }
            }
        }

        static String hex(byte[] bytes) {
            StringBuilder out = new StringBuilder(bytes.length * 2);
            for (byte b : bytes) {
                out.append(String.format("%02X", b & 0xFF));
            }
            return out.toString();
        }
    }
}
