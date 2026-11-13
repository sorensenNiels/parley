// Parley's SQL helper (R-SQL-3, D20): trusts a session's company CA certificate file (PEM) for
// TLS, besides Java's own trusted certificates, as Parley's 5250 connection does (D6).
// MIT OR Apache-2.0, as the rest of Parley.
package parley.sql;

import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.security.GeneralSecurityException;
import java.security.KeyStore;
import java.security.cert.Certificate;
import java.security.cert.CertificateFactory;
import java.security.cert.X509Certificate;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import javax.net.ssl.SSLContext;
import javax.net.ssl.TrustManager;
import javax.net.ssl.TrustManagerFactory;
import javax.net.ssl.X509TrustManager;

final class Trust {
    private Trust() {}

    /** Makes the certificates in `pemFile` trusted for this process, next to Java's own. */
    static void addCaFile(String pemFile) {
        try {
            Collection<? extends Certificate> extra;
            try (InputStream in = new FileInputStream(pemFile)) {
                extra = CertificateFactory.getInstance("X.509").generateCertificates(in);
            }
            if (extra.isEmpty()) {
                throw new IllegalArgumentException("the CA certificate file " + pemFile
                        + " holds no certificate");
            }
            KeyStore store = KeyStore.getInstance(KeyStore.getDefaultType());
            store.load(null, null);
            int n = 0;
            for (X509Certificate known : javaTrusted()) {
                store.setCertificateEntry("java-" + n++, known);
            }
            for (Certificate certificate : extra) {
                store.setCertificateEntry("session-" + n++, certificate);
            }
            TrustManagerFactory factory =
                    TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm());
            factory.init(store);
            SSLContext context = SSLContext.getInstance("TLS");
            context.init(null, factory.getTrustManagers(), null);
            SSLContext.setDefault(context);
        } catch (IOException | GeneralSecurityException e) {
            throw new IllegalArgumentException("cannot read the CA certificate file " + pemFile
                    + ": " + e.getMessage());
        }
    }

    /** The certificates Java trusts by itself. */
    private static List<X509Certificate> javaTrusted() throws GeneralSecurityException {
        TrustManagerFactory factory =
                TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm());
        factory.init((KeyStore) null);
        List<X509Certificate> all = new ArrayList<>();
        for (TrustManager manager : factory.getTrustManagers()) {
            if (manager instanceof X509TrustManager) {
                for (X509Certificate c : ((X509TrustManager) manager).getAcceptedIssuers()) {
                    all.add(c);
                }
            }
        }
        return all;
    }
}
