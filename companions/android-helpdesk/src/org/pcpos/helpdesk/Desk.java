package org.pcpos.helpdesk;

import android.app.Activity;
import android.os.*;
import android.content.*;
import android.net.Uri;
import android.graphics.Typeface;
import android.view.*;
import android.widget.*;
import org.json.*;
import java.io.*;
import java.net.*;
import java.time.Instant;
import java.util.concurrent.*;

public final class Desk extends Activity {
    private static final String ISSUE = "https://api.github.com/repos/KirtusJ/From-Prephysical-Hypotheses-to-Physical-Models/issues/270";
    private final Handler timer = new Handler(Looper.getMainLooper());
    private final ExecutorService io = Executors.newSingleThreadExecutor();
    private final Runnable tick = new Runnable() {
        public void run() { refresh(); timer.postDelayed(this, 120000); }
    };
    private LinearLayout pane;
    private TextView status;
    private boolean running, ledger;
    private String director = "";
    private int dp(int n) { return Math.round(getResources().getDisplayMetrics().density*n); }
    private TextView line(LinearLayout box, String s, int size, boolean bold) {
        TextView t = new TextView(this);
        t.setText(s); t.setTextSize(size); t.setTextColor(0xff19363d);
        if (bold) t.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
        t.setPadding(0,dp(5),0,dp(5)); box.addView(t); return t;
    }
    private void button(LinearLayout box,String title,Runnable action) {
        Button b=new Button(this); b.setText(title); b.setAllCaps(false);
        b.setOnClickListener(v->action.run()); box.addView(b);
    }
    public void onCreate(Bundle saved) {
        super.onCreate(saved);
        LinearLayout root = new LinearLayout(this); root.setOrientation(1);
        root.setFitsSystemWindows(true); root.setPadding(dp(16),dp(10),dp(16),dp(4));
        root.setBackgroundColor(0xfff5faf9); setContentView(root);
        line(root,"Director HelpDesk",30,true);
        LinearLayout bar=new LinearLayout(this); root.addView(bar);
        button(bar,"Exchange",()->{ledger=false;render();refresh();});
        button(bar,"Workers",()->{ledger=true;render();});
        button(bar,"Import",()->startActivityForResult(new Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE).setType("*/*"),1));
        status=line(root,"",14,false);
        ScrollView scroll=new ScrollView(this);root.addView(scroll);
        pane=new LinearLayout(this);pane.setOrientation(1);scroll.addView(pane);render();
    }
    protected void onResume(){super.onResume();timer.post(tick);}
    protected void onPause(){timer.removeCallbacks(tick);super.onPause();}
    protected void onDestroy(){io.shutdownNow();super.onDestroy();}
    private SharedPreferences cache(){return getSharedPreferences("monitor",MODE_PRIVATE);}
    private static String read(InputStream stream) throws IOException {
        try(InputStream in=stream;ByteArrayOutputStream out=new ByteArrayOutputStream()){
            byte[] buf=new byte[8192]; int n;
            while((n=in.read(buf))!=-1){if(out.size()+n>4*1024*1024)throw new IOException("File too large");out.write(buf,0,n);}
            return out.toString("UTF-8");
        }
    }
    private static String get(String url) throws IOException {
        HttpURLConnection c=(HttpURLConnection)new URL(url).openConnection();
        c.setConnectTimeout(10000);c.setReadTimeout(15000);c.setRequestProperty("User-Agent","PCP-Director-HelpDesk");
        try{return read(c.getInputStream());}finally{c.disconnect();}
    }
    private void refresh(){
        if(running)return;running=true;
        io.execute(()->{
            String error=null;
            try{
                int count=new JSONObject(get(ISSUE)).getInt("comments");
                int page=Math.max(1,(count+19)/20);
                JSONArray feed=new JSONArray();
                for(int p=Math.max(1,page-1);p<=page;p++){
                    JSONArray batch=new JSONArray(get(ISSUE+"/comments?per_page=20&page="+p));
                    for(int i=0;i<batch.length();i++)feed.put(batch.getJSONObject(i));
                }
                cache().edit().putString("feed",feed.toString()).putString("sync",Instant.now().toString()).apply();
            }catch(Exception e){error="Refresh failed. Showing last saved exchange.";}
            final String message=error;
            runOnUiThread(()->{running=false;if(isDestroyed())return;render();if(message!=null&&!ledger)status.append("\n"+message);});
        });
    }
    protected void onActivityResult(int req,int result,Intent data){
        super.onActivityResult(req,result,data);
        if(req!=1||result!=RESULT_OK||data==null||data.getData()==null)return;
        Uri uri=data.getData();
        io.execute(()->{try{
            JSONObject desk=new JSONObject(read(getContentResolver().openInputStream(uri)));
            JSONArray nodes=desk.getJSONArray("nodes");desk.getLong("revision");
            for(int i=0;i<nodes.length();i++){JSONObject n=nodes.getJSONObject(i);n.getLong("id");n.getString("label");n.getString("state");}
            cache().edit().putString("desk",desk.toString()).putString("imported",Instant.now().toString()).apply();
            runOnUiThread(()->{if(!isDestroyed()){ledger=true;render();}});
        }catch(Exception e){runOnUiThread(()->Toast.makeText(this,"Choose a Parrots status JSON file.",Toast.LENGTH_LONG).show());}});
    }
    private void render(){
        pane.removeAllViews();
        try{
            if(ledger){
                if(!cache().contains("desk")){status.setText("Import a Parrots status JSON file to see worker activity.");return;}
                JSONObject d=new JSONObject(cache().getString("desk","{}"));
                status.setText("Revision "+d.getLong("revision")+" · imported "+cache().getString("imported",""));
                JSONArray nodes=d.getJSONArray("nodes");
                for(int i=nodes.length()-1;i>=0;i--){JSONObject n=nodes.getJSONObject(i);
                    line(pane,"#"+n.getLong("id")+" · "+n.getString("state"),18,true);
                    line(pane,n.getString("label")+(n.isNull("parent")?"":" · parent #"+n.getLong("parent")),15,false);}
            }else{
                status.setText("Public director exchange · "+cache().getString("sync","awaiting first refresh"));
                JSONArray feed=new JSONArray(cache().getString("feed","[]"));
                LinearLayout filter=new LinearLayout(this);pane.addView(filter);
                for(String who:new String[]{"","Director G","Director Z"}){
                    button(filter,who.isEmpty()?"All":who,()->{director=who;render();});
                }
                int shown=0;
                for(int i=feed.length()-1;i>=Math.max(0,feed.length()-20);i--){
                    JSONObject c=feed.getJSONObject(i);String body=c.getString("body").trim();
                    if(!director.isEmpty() && !body.toLowerCase(java.util.Locale.ROOT)
                            .contains(director.toLowerCase(java.util.Locale.ROOT)))continue;
                    shown++;
                    line(pane,c.getString("created_at"),14,true);
                    String excerpt=body.contains("BEGIN AGE ENCRYPTED FILE")?"Encrypted check-in":body.substring(0,Math.min(body.length(),380));
                    TextView t=line(pane,excerpt,16,false);
                    Uri url=Uri.parse("https://github.com/KirtusJ/From-Prephysical-Hypotheses-to-Physical-Models/issues/270#issuecomment-"+c.getLong("id"));
                    t.setOnClickListener(v->startActivity(new Intent(Intent.ACTION_VIEW,url)));
                }
                if(shown==0)line(pane, director.isEmpty()?"No check-ins loaded.":"No recent public check-ins mention "+director+".",16,false);
            }
        }catch(JSONException e){status.setText("Saved data could not be displayed. Refresh or import again.");}
    }
}
