package vvanders.com.simplelink.link;

import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothSocket;
import android.os.Handler;
import android.os.Looper;
import android.os.Message;

import java.io.IOException;
import java.util.UUID;

import vvanders.com.simplelink.SimpleLink;

public class LinkThread {
    private SimpleLink mLink = new SimpleLink();
    private boolean mInitialized = false;
    private Thread mThread;
    private Handler mHandler;
    private Handler mComHandler;
    private ConnectionStatus mConnectCallback;
    private LinkStatus mCallback;

    private BluetoothDevice mBTDevice;
    private BluetoothSocket mBTSocket;

    private long mLastTick = 0;

    private final int MSG_STOP = 1;
    private final int MSG_SET_CALLSIGN = 2;
    private final int MSG_SET_CALLBACK = 3;
    private final int MSG_CONNECT_LOOPBACK = 4;
    private final int MSG_CONNECT_BLUETOOTH = 5;
    private final int MSG_TICK = 6;
    private final int MSG_SEND = 7;

    private final UUID BLUETOOTH_SPP_UUID = UUID
            .fromString("00001101-0000-1000-8000-00805F9B34FB");

    private final long TICK_INTERVAL = 33;

    private static final class SendMessage {
        public final int[] Route;
        public final byte[] Data;

        public SendMessage(int[] route, byte[] data) {
            Route = route;
            Data = data;
        }
    }

    private void internalConnectBluetooth(String deviceName) {
        mBTDevice = getBluetoothDevice(deviceName);

        if(mBTDevice == null) {
            notifyConnectionFailure("Unable to find bluetooth device " + deviceName);
            return;
        }

        try {
            mBTSocket = mBTDevice.createRfcommSocketToServiceRecord(BLUETOOTH_SPP_UUID);

            if (mBTSocket == null) {
                notifyConnectionFailure("Unable to open SPP socket");
                return;
            }

            mBTSocket.connect();

            if (mBTSocket.isConnected()) {
                mLink.set_rx_tx(mBTSocket.getInputStream(), mBTSocket.getOutputStream());
                notifyConnectionSuccess();
            }
        } catch (IOException e) {
            notifyConnectionFailure("IO error while connecting " + e.getMessage());
            return;
        }
    }

    private synchronized void notifyConnectionSuccess() {
        if(mConnectCallback != null) {
            mComHandler.post(new Runnable() {
                @Override
                public void run() {
                    mConnectCallback.onSuccess();
                }
            });
        }
    }

    private synchronized void notifyConnectionFailure(final String message) {
        if(mConnectCallback != null) {
            mComHandler.post(new Runnable() {
                @Override
                public void run() {
                    mConnectCallback.onError(message);
                }
            });
        }
    }

    private synchronized void dispatchCallback(Runnable run) {
        mComHandler.post(run);
    }

    private BluetoothDevice getBluetoothDevice(String deviceName) {
        for(BluetoothDevice device : BluetoothAdapter.getDefaultAdapter().getBondedDevices()) {
            if(device.getName().equals(deviceName)) {
                return device;
            }
        }

        return null;
    }

    public interface ConnectionStatus {
        void onSuccess();
        void onError(String message);
    }

    public interface LinkStatus {
        void Recv(int prn, int[] route, byte[] data);
        void Ack(int prn);
        void Observe(int prn, int[] route, byte[] data);
        void Retry(int prn, int next_retry_ms);
        void Expire(int prn);
        void Send(int prn, int[] route, byte[] data);
    }

    public void start(Handler comHandler) {
        mComHandler = comHandler;

        mThread = new Thread(new Runnable() {
            @Override
            public void run() {
                Looper.prepare();

                synchronized (this) {
                    mHandler = new Handler() {
                        public void handleMessage(Message msg) {
                            switch(msg.what) {
                                case MSG_STOP:
                                    Looper.myLooper().quitSafely();
                                    break;

                                case MSG_SET_CALLSIGN:
                                    mLink.init((String)msg.obj);
                                    mInitialized = true;
                                    break;

                                case MSG_SET_CALLBACK:
                                    final LinkStatus callback = (LinkStatus)msg.obj;
                                    mCallback = callback;
                                    mLink.set_callback(new SimpleLink.Callback() {
                                        @Override
                                        public void Recv(final int prn, final int[] route, final byte[] data) {
                                            dispatchCallback(new Runnable() {
                                                @Override
                                                public void run() {
                                                    callback.Recv(prn, route, data);
                                                }
                                            });
                                        }

                                        @Override
                                        public void Ack(final int prn) {
                                            dispatchCallback(new Runnable() {
                                                @Override
                                                public void run() {
                                                    callback.Ack(prn);
                                                }
                                            });
                                        }

                                        @Override
                                        public void Observe(final int prn, final int[] route, final byte[] data) {
                                            dispatchCallback(new Runnable() {
                                                @Override
                                                public void run() {
                                                    callback.Observe(prn, route, data);
                                                }
                                            });
                                        }

                                        @Override
                                        public void Retry(final int prn, final int next_retry_ms) {
                                            dispatchCallback(new Runnable() {
                                                @Override
                                                public void run() {
                                                    callback.Retry(prn, next_retry_ms);
                                                }
                                            });
                                        }

                                        @Override
                                        public void Expire(final int prn) {
                                            dispatchCallback(new Runnable() {
                                                @Override
                                                public void run() {
                                                    callback.Expire(prn);
                                                }
                                            });
                                        }
                                    });
                                    break;

                                case MSG_CONNECT_LOOPBACK:
                                    mLink.open_loopback();
                                    notifyConnectionSuccess();
                                    break;

                                case MSG_CONNECT_BLUETOOTH:
                                    internalConnectBluetooth((String)msg.obj);
                                    break;

                                case MSG_TICK:
                                    if(mInitialized) {
                                        long delta = System.currentTimeMillis() - mLastTick;
                                        mLastTick = System.currentTimeMillis();

                                        mLink.tick((int) delta);

                                        //Re-enqueue tick
                                        mHandler.sendMessageDelayed(Message.obtain(mHandler, MSG_TICK), TICK_INTERVAL);
                                    }
                                    break;

                                case MSG_SEND:
                                    final SendMessage sm = (SendMessage)msg.obj;
                                    final int prn = mLink.send(sm.Route, sm.Data);

                                    if(prn == 0) {
                                        notifyConnectionFailure("Error sending message");
                                    } else {
                                        dispatchCallback(new Runnable() {
                                            @Override
                                            public void run() {
                                                mCallback.Send(prn, sm.Route, sm.Data);
                                            }
                                        });
                                    }
                                    break;
                            }
                        }
                    };
                }

                Looper.loop();
            }
        });

        mThread.start();
    }

    public synchronized  void init(String callsign, LinkStatus callback, ConnectionStatus connectionCallback) {
        mConnectCallback = connectionCallback;

        mHandler.sendMessage(Message.obtain(mHandler, MSG_SET_CALLSIGN, callsign));
        mHandler.sendMessage(Message.obtain(mHandler, MSG_SET_CALLBACK, callback));

        //Kick off initial tick
        mHandler.sendMessageDelayed(Message.obtain(mHandler, MSG_TICK), TICK_INTERVAL);
    }

    public synchronized void connect_loopback() {
        mHandler.sendMessage(Message.obtain(mHandler, MSG_CONNECT_LOOPBACK));
    }

    public synchronized void connect_spp(String deviceName) {
        mHandler.sendMessage(Message.obtain(mHandler, MSG_CONNECT_BLUETOOTH, deviceName));
    }

    public synchronized void send(final int[] route, final byte[] data) {
        mHandler.sendMessage(Message.obtain(mHandler, MSG_SEND, new SendMessage(route, data)));
    }

    public synchronized void stop() {
        mHandler.sendMessage(Message.obtain(mHandler, MSG_STOP));
    }
}
