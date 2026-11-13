// Parley's SQL helper: checks of what needs no IBM i, run by `make helper` (R-SQL-11).
// MIT OR Apache-2.0, as the rest of Parley.
package parley.sql;

import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

final class SelfTest {
    private SelfTest() {}

    private static int failures;

    static int run() {
        jsonReadsWhatParleySends();
        jsonWritesWhatParleyReads();
        brokenJsonIsAnErrorNotACrash();
        binaryIsHex();
        timesKeepTheirSeconds();
        if (failures == 0) {
            System.out.println("parley-sql self-test: ok");
        }
        return failures == 0 ? 0 : 1;
    }

    private static void check(boolean ok, String what) {
        if (!ok) {
            failures++;
            System.out.println("parley-sql self-test FAILED: " + what);
        }
    }

    private static void jsonReadsWhatParleySends() {
        Map<String, Object> request = Helper.asMap(Json.parse(
                "{\"id\":3,\"op\":\"run\",\"sql\":\"SELECT 'æøå' FROM \\\"T\\\"\\n\","
                        + "\"max_rows\":1000,\"tls\":true,\"ca_file\":null,\"x\":[1,2.5,\"\\u00e6\"]}"));
        check(Long.valueOf(3).equals(request.get("id")), "id");
        check("SELECT 'æøå' FROM \"T\"\n".equals(request.get("sql")), "sql with quotes and æøå");
        check(Boolean.TRUE.equals(request.get("tls")), "true");
        check(request.containsKey("ca_file") && request.get("ca_file") == null, "null");
        check(Arrays.asList(1L, 2.5, "æ").equals(request.get("x")), "array");
    }

    private static void jsonWritesWhatParleyReads() {
        Map<String, Object> answer = new LinkedHashMap<>();
        answer.put("id", 7L);
        answer.put("rows", Arrays.asList(Arrays.asList("a\"b", null, "12345678901234567890.123"), List.of()));
        answer.put("more", false);
        String text = Json.write(answer);
        check(text.equals("{\"id\":7,\"rows\":[[\"a\\\"b\",null,\"12345678901234567890.123\"],[]],"
                + "\"more\":false}"), "written: " + text);
        check(Json.parse(text).equals(answer), "read back");
        check(Json.write("line\nbreak\u0001").equals("\"line\\nbreak\\u0001\""), "control characters");
    }

    private static void brokenJsonIsAnErrorNotACrash() {
        for (String broken : new String[] {"", "{", "{\"id\":", "[1,]", "\"open", "{\"a\":1}x", "tru"}) {
            try {
                Json.parse(broken);
                check(false, "accepted " + broken);
            } catch (IllegalArgumentException expected) {
                check(expected.getMessage().startsWith("invalid JSON"), "message for " + broken);
            }
        }
    }

    private static void binaryIsHex() {
        check(Helper.Values.hex(new byte[] {0, (byte) 0xC1, 0x7F}).equals("00C17F"), "hex");
    }

    private static void timesKeepTheirSeconds() {
        check(Helper.Values.TIME.format(java.time.LocalTime.of(9, 43)).equals("09:43:00"), "time");
        check(Helper.Values.TIMESTAMP.format(java.time.LocalDateTime.of(2026, 11, 13, 9, 43, 7, 1000))
                .equals("2026-11-13 09:43:07.000001"), "timestamp");
    }
}
