package vvanders.com.simplelink.link;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.ArrayList;

public class LoopBackStream {
    private ArrayList<Integer> m_data = new ArrayList<>();

    public InputStream getIs() {
        return new InputStream() {
            @Override
            public int read() throws IOException {
                if (m_data.size() > 0) {
                    return m_data.remove(0);
                } else {
                    return -1;
                }
            }

            @Override
            public int available() {
                return m_data.size();
            }
        };
    }

    public OutputStream getOs() {
        return new OutputStream() {
            @Override
            public void write(int b) throws IOException {
                //Convert from -128->127 to 0->255
                m_data.add(new Integer(b + 128));
            }
        };
    }
}
