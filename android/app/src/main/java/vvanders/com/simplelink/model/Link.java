package vvanders.com.simplelink.model;

import java.nio.charset.StandardCharsets;
import java.util.ArrayList;

public class Link {
    private ArrayList<Message> mMessages = new ArrayList<>();

    public void addMessage(int prn, long timestamp, int[] route, byte[] data, Message.SourceType source) {
        Message msg = new Message(prn, timestamp, Route.FromLink(route), new String(data, StandardCharsets.UTF_8), source);

        mMessages.add(msg);
    }

    public void ack(int prn) {
        for(Message msg : mMessages) {
            if(msg.PRN == prn) {
                msg.Acked = true;
                return;
            }
        }
    }

    public ArrayList<Message> getMessages() {
        return mMessages;
    }
}
