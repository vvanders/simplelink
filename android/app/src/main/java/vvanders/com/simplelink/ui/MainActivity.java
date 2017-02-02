package vvanders.com.simplelink.ui;

import android.app.AlertDialog;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.content.DialogInterface;
import android.support.v7.app.AppCompatActivity;
import android.os.Bundle;
import android.util.Log;
import android.view.View;
import android.widget.ArrayAdapter;
import android.widget.Button;
import android.widget.EditText;
import android.widget.ListView;
import android.widget.RadioGroup;
import android.widget.Spinner;
import android.widget.TextView;

import java.nio.charset.StandardCharsets;
import java.util.Set;

import vvanders.com.simplelink.link.LinkThread;
import vvanders.com.simplelink.R;
import vvanders.com.simplelink.SimpleLink;
import vvanders.com.simplelink.SimpleLinkApplication;
import vvanders.com.simplelink.model.Link;
import vvanders.com.simplelink.model.Message;

public class MainActivity extends AppCompatActivity {
    private LinkThread mLink;
    private final MessageAdapater mAdapter = new MessageAdapater();
    private Link mModel = new Link();

    private enum Source {
        Bluetooth,
        Loopback
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);

        mLink = ((SimpleLinkApplication)getApplication()).getLink();

        ListView messages = (ListView)findViewById(R.id.list_msg);
        messages.setAdapter(mAdapter);

        final EditText routeText = (EditText)findViewById(R.id.route_text);
        final EditText msgText = (EditText)findViewById(R.id.msg_text);

        Button sendButton = (Button)findViewById(R.id.send_button);
        sendButton.setOnClickListener(new View.OnClickListener() {
            @Override
            public void onClick(View v) {
                send(routeText.getText().toString(), msgText.getText().toString());
                msgText.setText("");
            }
        });

        AlertDialog dialog = createConnectDialog();
        dialog.show();
    }

    private AlertDialog createConnectDialog() {
        AlertDialog.Builder builder = new AlertDialog.Builder(this);
        final View inner = getLayoutInflater().inflate(R.layout.dialog_connect, null);

        Set<BluetoothDevice> devices = BluetoothAdapter.getDefaultAdapter().getBondedDevices();
        String[] items = new String[devices.size()];

        int idx = 0;
        for(BluetoothDevice device : devices) {
            items[idx++] = device.getName();
        }

        final Spinner btSpinner = (Spinner)inner.findViewById(R.id.spinner_bt);
        btSpinner.setAdapter(new ArrayAdapter<String>(this, android.R.layout.simple_spinner_dropdown_item, items));

        final EditText callsignText = (EditText)inner.findViewById(R.id.text_callsign);
        final RadioGroup rgSource = (RadioGroup)inner.findViewById(R.id.radiogroup_source);

        builder.setView(inner)
            .setTitle("Connect")
            .setPositiveButton("Connect", new DialogInterface.OnClickListener() {
                @Override
                public void onClick(DialogInterface dialog, int which) {
                    String sourceTarget = "";
                    Source source = Source.Loopback;
                    switch(rgSource.getCheckedRadioButtonId()) {
                        case R.id.radio_loopback:
                            source = Source.Loopback;
                            break;

                        case R.id.radio_bt:
                            source = Source.Bluetooth;
                            sourceTarget = (String)btSpinner.getSelectedItem();
                            break;
                    }

                    init(callsignText.getText().toString(), source, sourceTarget);
                }
            })
            .setNegativeButton("Cancel", new DialogInterface.OnClickListener() {
                @Override
                public void onClick(DialogInterface dialog, int which) {
                    finish();
                }
            });

        return builder.create();
    }

    private void send(String route, String msg) {
        String[] routes = route.split("\\s+");
        int[] translated = new int[routes.length];

        for(int i = 0; i < routes.length; ++i) {
            translated[i] = SimpleLink.encode_addr(routes[i]);
        }

        byte[] data = msg.getBytes(StandardCharsets.UTF_8);
        mLink.send(translated, data);
    }

    private void init(String callsign, Source source, String sourceTarget) {
        mLink.init(callsign, new LinkThread.LinkStatus() {
                    @Override
                    public void Recv(int prn, int[] route, byte[] data) {
                        mModel.addMessage(prn, System.currentTimeMillis(), route, data, Message.SourceType.Received);
                        mAdapter.updateData(mModel);
                    }

                    @Override
                    public void Ack(int prn) {
                        mModel.ack(prn);
                        mAdapter.updateData(mModel);
                    }

                    @Override
                    public void Observe(int prn, int[] route, byte[] data) {
                        mModel.addMessage(prn, System.currentTimeMillis(), route, data, Message.SourceType.Observed);
                        mAdapter.updateData(mModel);
                    }

                    @Override
                    public void Retry(int prn, int next_retry_ms) {
                        Log.i("VALLOG", "Retry");
                    }

                    @Override
                    public void Expire(int prn) {
                        Log.i("VALLOG", "Expire");
                    }

                    @Override
                    public void Send(int prn, int[] route, byte[] data) {
                        mModel.addMessage(prn, System.currentTimeMillis(), route, data, Message.SourceType.Sent);
                        mAdapter.updateData(mModel);
                    }
                },
                new LinkThread.ConnectionStatus() {
                    @Override
                    public void onSuccess() {

                    }
                    @Override
                    public void onError(String message) {

                    }
                });

        switch (source) {
            case Loopback:
                mLink.connect_loopback();
                break;

            case Bluetooth:
                mLink.connect_spp(sourceTarget);
                break;
        }

        mAdapter.updateData(mModel);
    }
}
