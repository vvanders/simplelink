package vvanders.com.simplelink.ui;

import android.content.Context;
import android.database.DataSetObserver;
import android.view.LayoutInflater;
import android.view.View;
import android.view.ViewGroup;
import android.widget.ListAdapter;
import android.widget.TextView;

import java.util.ArrayList;

import vvanders.com.simplelink.R;
import vvanders.com.simplelink.model.Link;
import vvanders.com.simplelink.model.Message;


public class MessageAdapater implements ListAdapter {
    private Link mSource;
    final private ArrayList<DataSetObserver> mObservers = new ArrayList<>();

    public MessageAdapater() {
    }

    public void updateData(Link source) {
        mSource = source;

        for(DataSetObserver obs : mObservers) {
            obs.onChanged();
        }
    }

    @Override
    public boolean areAllItemsEnabled() {
        return false;
    }

    @Override
    public boolean isEnabled(int position) {
        return false;
    }

    @Override
    public void registerDataSetObserver(DataSetObserver observer) {
        mObservers.add(observer);
    }

    @Override
    public void unregisterDataSetObserver(DataSetObserver observer) {
        mObservers.remove(observer);
    }

    @Override
    public int getCount() {
        if(mSource != null) {
            return mSource.getMessages().size();
        }

        return 0;
    }

    @Override
    public Object getItem(int position) {
        return mSource.getMessages().get(position);
    }

    @Override
    public long getItemId(int position) {
        return mSource.getMessages().get(position).PRN;
    }

    @Override
    public boolean hasStableIds() {
        return true;
    }

    @Override
    public View getView(int position, View convertView, ViewGroup parent) {
        if(convertView == null) {
            convertView = ((LayoutInflater)parent.getContext().getSystemService(Context.LAYOUT_INFLATER_SERVICE)).inflate(R.layout.message_item, null);
        }

        Message msg = mSource.getMessages().get(position);

        String sourceText = "";
        switch(msg.Source) {
            case Observed:
                sourceText = "O";
                break;

            case Received:
                sourceText = "R";
                break;

            case Sent:
                sourceText = "S";
                break;
        }

        TextView content = (TextView)convertView.findViewById(R.id.text_content);
        content.setText(sourceText + " " + msg.Content);

        return convertView;
    }

    @Override
    public int getItemViewType(int position) {
        return 0;
    }

    @Override
    public int getViewTypeCount() {
        return 1;
    }

    @Override
    public boolean isEmpty() {
        return false;
    }
}
