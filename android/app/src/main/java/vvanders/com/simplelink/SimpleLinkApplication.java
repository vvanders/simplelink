package vvanders.com.simplelink;

import android.app.Application;
import android.os.Handler;

import vvanders.com.simplelink.link.LinkThread;

public class SimpleLinkApplication extends Application {
    private LinkThread mLinkThread;

    @Override
    public void onCreate() {
        super.onCreate();

        mLinkThread = new LinkThread();
        mLinkThread.start(new Handler(getMainLooper()));
    }

    @Override
    public void onTerminate() {
        super.onTerminate();
    }

    public LinkThread getLink() {
        return mLinkThread;
    }
}
