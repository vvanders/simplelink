package vvanders.com.simplelink;

import java.io.InputStream;
import java.io.OutputStream;
import java.nio.ByteBuffer;

/**
 * Created by valer on 1/5/2017.
 */

public class SimpleLink {
    private long m_link = 0;

    private native static void static_init();

    public interface Callback {
        void Recv(int prn, int[] route, byte[] data);
        void Ack(int prn);
        void Observe(int prn, int[] route, byte[] data);
    }

    public native boolean init(String callsign);
    public native boolean open_loopback();

    public void register_rx_tx(InputStream is, OutputStream os) {

    }

    static {
        System.loadLibrary("slink_android");
        static_init();
    }
}
