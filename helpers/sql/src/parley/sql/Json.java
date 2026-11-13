// Parley's SQL helper (R-SQL-11, D20): a small JSON reader and writer, since Java 11 has none.
// Values: Map<String, Object> (objects), List<Object> (arrays), String, Long or Double (numbers),
// Boolean, and null. MIT OR Apache-2.0, as the rest of Parley.
package parley.sql;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

final class Json {
    private final String text;
    private int at;

    private Json(String text) {
        this.text = text;
    }

    /** Reads one JSON value that fills `text`; an IllegalArgumentException names the problem. */
    static Object parse(String text) {
        Json json = new Json(text);
        json.blanks();
        Object value = json.value();
        json.blanks();
        if (json.at != text.length()) {
            throw json.error("text after the value");
        }
        return value;
    }

    private Object value() {
        if (at >= text.length()) {
            throw error("the end where a value was due");
        }
        char c = text.charAt(at);
        switch (c) {
            case '{':
                return object();
            case '[':
                return array();
            case '"':
                return string();
            case 't':
                return word("true", Boolean.TRUE);
            case 'f':
                return word("false", Boolean.FALSE);
            case 'n':
                return word("null", null);
            default:
                if (c == '-' || (c >= '0' && c <= '9')) {
                    return number();
                }
                throw error("'" + c + "' where a value was due");
        }
    }

    private Map<String, Object> object() {
        Map<String, Object> map = new LinkedHashMap<>();
        at++; // {
        blanks();
        if (peek('}')) {
            at++;
            return map;
        }
        while (true) {
            blanks();
            if (!peek('"')) {
                throw error("a name where one was due");
            }
            String name = string();
            blanks();
            expect(':');
            blanks();
            map.put(name, value());
            blanks();
            if (peek(',')) {
                at++;
                continue;
            }
            expect('}');
            return map;
        }
    }

    private List<Object> array() {
        List<Object> list = new ArrayList<>();
        at++; // [
        blanks();
        if (peek(']')) {
            at++;
            return list;
        }
        while (true) {
            blanks();
            list.add(value());
            blanks();
            if (peek(',')) {
                at++;
                continue;
            }
            expect(']');
            return list;
        }
    }

    private String string() {
        at++; // "
        StringBuilder out = new StringBuilder();
        while (at < text.length()) {
            char c = text.charAt(at++);
            if (c == '"') {
                return out.toString();
            }
            if (c != '\\') {
                out.append(c);
                continue;
            }
            if (at >= text.length()) {
                break;
            }
            char e = text.charAt(at++);
            switch (e) {
                case '"': out.append('"'); break;
                case '\\': out.append('\\'); break;
                case '/': out.append('/'); break;
                case 'b': out.append('\b'); break;
                case 'f': out.append('\f'); break;
                case 'n': out.append('\n'); break;
                case 'r': out.append('\r'); break;
                case 't': out.append('\t'); break;
                case 'u':
                    if (at + 4 > text.length()) {
                        throw error("a short \\u escape");
                    }
                    try {
                        out.append((char) Integer.parseInt(text.substring(at, at + 4), 16));
                    } catch (NumberFormatException x) {
                        throw error("a \\u escape that is not hex");
                    }
                    at += 4;
                    break;
                default:
                    throw error("the escape \\" + e);
            }
        }
        throw error("a string without its closing quote");
    }

    private Object number() {
        int start = at;
        if (peek('-')) {
            at++;
        }
        boolean fraction = false;
        while (at < text.length()) {
            char c = text.charAt(at);
            if (c >= '0' && c <= '9') {
                at++;
            } else if (c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-') {
                fraction = true;
                at++;
            } else {
                break;
            }
        }
        String digits = text.substring(start, at);
        try {
            return fraction ? (Object) Double.parseDouble(digits) : (Object) Long.parseLong(digits);
        } catch (NumberFormatException x) {
            throw error("the number " + digits);
        }
    }

    private Object word(String word, Object value) {
        if (!text.startsWith(word, at)) {
            throw error("a word that is not true, false or null");
        }
        at += word.length();
        return value;
    }

    private void blanks() {
        while (at < text.length() && " \t\r\n".indexOf(text.charAt(at)) >= 0) {
            at++;
        }
    }

    private boolean peek(char c) {
        return at < text.length() && text.charAt(at) == c;
    }

    private void expect(char c) {
        if (!peek(c)) {
            throw error("'" + c + "' was due");
        }
        at++;
    }

    private IllegalArgumentException error(String what) {
        return new IllegalArgumentException("invalid JSON at character " + at + ": " + what);
    }

    /** `value` as compact JSON on one line. */
    static String write(Object value) {
        StringBuilder out = new StringBuilder();
        write(out, value);
        return out.toString();
    }

    @SuppressWarnings("unchecked")
    private static void write(StringBuilder out, Object value) {
        if (value == null) {
            out.append("null");
        } else if (value instanceof String) {
            quote(out, (String) value);
        } else if (value instanceof Boolean || value instanceof Long || value instanceof Integer) {
            out.append(value);
        } else if (value instanceof Number) {
            double d = ((Number) value).doubleValue();
            out.append(Double.isFinite(d) ? Double.toString(d) : "null");
        } else if (value instanceof Map) {
            out.append('{');
            boolean first = true;
            for (Map.Entry<String, Object> e : ((Map<String, Object>) value).entrySet()) {
                if (!first) {
                    out.append(',');
                }
                first = false;
                quote(out, e.getKey());
                out.append(':');
                write(out, e.getValue());
            }
            out.append('}');
        } else if (value instanceof List) {
            out.append('[');
            boolean first = true;
            for (Object item : (List<Object>) value) {
                if (!first) {
                    out.append(',');
                }
                first = false;
                write(out, item);
            }
            out.append(']');
        } else {
            quote(out, value.toString());
        }
    }

    private static void quote(StringBuilder out, String s) {
        out.append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '"': out.append("\\\""); break;
                case '\\': out.append("\\\\"); break;
                case '\n': out.append("\\n"); break;
                case '\r': out.append("\\r"); break;
                case '\t': out.append("\\t"); break;
                default:
                    if (c < 0x20 || c == 0x2028 || c == 0x2029) {
                        out.append(String.format("\\u%04x", (int) c));
                    } else {
                        out.append(c);
                    }
            }
        }
        out.append('"');
    }
}
