package vvanders.com.simplelink.model;

public class Message {
    public enum SourceType {
        Observed,
        Sent,
        Received
    }

    public Message(int prn, long timestamp, Route route, String content, SourceType source) {
        PRN = prn;
        Route = route;
        Content = content;
        Source = source;
        Timestamp = timestamp;
    }

    public final int PRN;
    public final Route Route;
    public final String Content;
    public final SourceType Source;
    public final long Timestamp;

    public boolean Acked = false;
}
