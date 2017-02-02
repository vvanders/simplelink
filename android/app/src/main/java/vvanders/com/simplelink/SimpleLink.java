package vvanders.com.simplelink;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;

import vvanders.com.simplelink.link.LoopBackStream;

public class SimpleLink {
    static boolean s_init = false;

    private long m_link = 0;
    private InputStream m_rx;
    private OutputStream m_tx;
    private Callback m_callback;
    private byte[] m_scratch = new byte[512];

    private native static void static_init();

    private int internal_fill_read(int max) {
        try {
            int to_read = Math.min(max, m_rx.available());
            return m_rx.read(m_scratch);
        } catch (IOException e) {
            return -1;
        }
    }

    private int internal_flush_write(int end) {
        try {
            m_tx.write(m_scratch, 0, end);
            return 0;
        } catch (IOException e) {
            return -1;
        }
    }

    private void internal_recv(int prn, int[] route, byte[] data) {
        if(m_callback != null) {
            m_callback.Recv(prn, route, data);
        }
    }

    private void internal_ack(int prn) {
        if(m_callback != null) {
            m_callback.Ack(prn);
        }
    }

    private void internal_observe(int prn, int [] route, byte[] data) {
        if(m_callback != null) {
            m_callback.Observe(prn, route, data);
        }
    }

    private void internal_retry(int prn, int next_retry_ms) {
        if(m_callback != null) {
            m_callback.Retry(prn, next_retry_ms);
        }
    }

    private void internal_expire(int prn) {
        if(m_callback != null) {
            m_callback.Expire(prn);
        }
    }

    public static native int encode_addr(String addr);
    public static native String decode_addr(int addr);

    public interface Callback {
        void Recv(final int prn, final int[] route, final byte[] data);
        void Ack(final int prn);
        void Observe(final int prn, final int[] route, final byte[] data);
        void Retry(final int prn, final int next_retry_ms);
        void Expire(final int prn);
    }

    public boolean init(String callsign) {
        if(!s_init) {
            static_init();
            s_init = true;
        }

        return internal_init(callsign);
    }
    public native boolean internal_init(String callsign);

    public void open_loopback() {
        LoopBackStream ls = new LoopBackStream();
        set_rx_tx(ls.getIs(), ls.getOs());
    }

    public void set_rx_tx(InputStream rx, OutputStream tx) {
        m_rx = rx;
        m_tx = tx;
    }

    public void set_callback(Callback callback) {
        m_callback = callback;
    }

    public native boolean tick(int elapsedMs);

    public native int send(int[] route, byte[] data);

    static {
        System.loadLibrary("slink_android");
    }
}
