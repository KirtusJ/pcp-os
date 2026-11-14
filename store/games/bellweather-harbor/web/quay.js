// The desk remembers which profile is being read; coins, cargo and keepsakes
// live in the shared Rust ledger and can also be opened by the native game.
const quayDesk={person:0,asking:false,shown:null,portrait:'',mini:'',memories:'',content:''};
let quayStill='',quayAnswerUntil=0,quayOpener=null,quayReceiptTide=null;
const quayDecoder=new TextDecoder('utf-8');
const quayLines=new Map();

function quayLine(field,person=0,slot=0){
  const ptr=wasm.quay_text_ptr(field,person,slot);
  // These pointers refer to immutable str constants in the WASM image.
  if(!quayLines.has(ptr)){
    const length=wasm.quay_text_len(field,person,slot);
    quayLines.set(ptr,quayDecoder.decode(new Uint8Array(wasm.memory.buffer,ptr,length)));
  }
  return quayLines.get(ptr);
}
function paintNeighbor(id,person){
  const canvas=$(id),ctx=canvas.getContext('2d',{alpha:false});
  const ptr=wasm.quay_portrait(person);
  blitRgb(new Uint32Array(wasm.memory.buffer,ptr,80*64),ctx.createImageData(80,64),ctx);
}
function selectNeighbor(person){
  quayDesk.person=person;quayDesk.asking=false;quayDesk.shown=null;quayDesk.content='';
  $('quay-memory').open=false;
  setText($('quay-response'),'');
  renderQuay();
}
function closeQuay(){
  const book=$('quay-book');
  if(book.open)book.close();
  (quayOpener||$('quay-open')).focus();
}
function initQuay(){
  const book=$('quay-book');
  for(let i=0;i<3;i++){
    const button=document.createElement('button');
    button.type='button';button.id='quay-neighbor-'+i;
    button.append(document.createElement('span'),document.createElement('small'));
    button.onclick=()=>{actStart();selectNeighbor(i)};
    $('quay-roster').appendChild(button);
  }
  function visit(e){
    quayOpener=e.currentTarget;
    if(!wasm)return;
    actStart();book.showModal();
    $('quay-overheard').open=false;
    selectNeighbor(wasm.quay_visitor());book.scrollTop=0;$('quay-close').focus();
  };
  $('quay-open').onclick=visit;
  $('quay-contact').onclick=visit;
  $('quay-close').onclick=closeQuay;
  book.addEventListener('cancel',e=>{e.preventDefault();closeQuay()});
  book.addEventListener('keydown',e=>{
    if(e.key==='Escape'&&book.open){e.preventDefault();e.stopPropagation();closeQuay()}
  });
  book.addEventListener('click',e=>{
    if(e.target!==book)return;
    const box=book.getBoundingClientRect();
    if(e.clientX<box.left||e.clientX>box.right||e.clientY<box.top||e.clientY>box.bottom)closeQuay();
  });
  $('quay-ask').onclick=()=>{
    actStart();
    if(quayDesk.person!==wasm.quay_visitor()){
      selectNeighbor(wasm.quay_visitor());$('quay-ask').focus();return;
    }
    quayDesk.asking=!quayDesk.asking;
    renderQuay();
  };
  $('quay-give').onclick=()=>{
    const seen=quayDesk.shown;
    if(!seen)return;
    actStart();
    const paid=wasm.quay_help(seen.tide,seen.person,seen.tier);
    if(paid){
      quayReceiptTide=seen.tide;
      const count=wasm.quay_helped(seen.person);
      const keepsake=count<=3?' Keepsake received: '+quayLine(7,seen.person,count-1)+'.':'';
      const lanterns=wasm.quay_lanterns()&&!seen.lanterns?' The neighbors will sail their lanterns at dusk.':'';
      setText($('quay-response'),'Received '+seen.reward+' coins.'+keepsake+lanterns);
      if(count<=3)$('quay-memory').open=true;
      sound([392,494,587]);note(quayLine(0,seen.person)+' paid '+seen.reward+' coins for the delivery.');
    }else{
      setText($('quay-response'),seen.tide!==wasm.freight_tide()?'The watch moved on before the handover. Your cargo is still aboard.':'Those terms are no longer available. Check the current request.');
    }
    finishAction();
    if($('quay-give').disabled)$('quay-ask').focus();
  };
}
function renderQuay(){
  const tide=wasm.freight_tide(),watch=wasm.quay_watch(),visitor=wasm.quay_visitor();
  if(quayReceiptTide!==null&&quayReceiptTide!==tide){
    quayReceiptTide=null;setText($('quay-response'),'');
  }
  const minutes=wasm.quay_minutes();
  const clock=String(Math.floor(minutes/60)).padStart(2,'0')+':'+String(minutes%60).padStart(2,'0');
  const watchName=quayLine(11),weather=quayLine(12),visitorName=quayLine(0,visitor);
  setText($('quay-clock'),clock);setText($('quay-watch'),watchName);setText($('quay-weather'),weather);
  setText($('quay-present'),visitorName);
  const label='On the quay. Visit '+visitorName+'.';
  $('quay-open').setAttribute('aria-label',label);
  const contact=$('quay-contact');
  contact.setAttribute('aria-label','Visit '+visitorName+' on the pier.');
  const anchor=wasm.quay_contact_point();
  contact.style.left=(100*(anchor&65535)/480)+'%';
  contact.style.top=(100*(anchor>>>16)/192)+'%';
  const mini=visitor+'/'+watch;
  if(mini!==quayDesk.mini){paintNeighbor('quay-mini',visitor);quayDesk.mini=mini}
  if(!$('quay-book').open)return;
  const p=quayDesk.person,isHere=p===visitor,served=Boolean(wasm.quay_served()),tier=wasm.quay_tier();
  const visits=wasm.quay_visits(p),helped=wasm.quay_helped(p);
  const lanterns=Boolean(wasm.quay_lanterns()),sailing=Boolean(wasm.quay_sailing());
  const quantities=wireGoods.map((_,g)=>wasm.quay_cargo(g)),reward=wasm.quay_reward();
  const stock=quantities.map((n,g)=>n?wasm.market_held(g):0);
  if(quayDesk.asking&&isHere&&!served){
    setText($('quay-departure'),'Leaves in '+Math.ceil(wasm.freight_tide_ms()/1000)+'s');
  }
  const greeting=quayLine(3,p);
  const signature=[p,tide,helped,served,tier,stock,lanterns,greeting,quayDesk.asking].join('/');
  if(signature===quayDesk.content)return;
  quayDesk.content=signature;
  for(let i=0;i<3;i++){
    const b=$('quay-neighbor-'+i);
    setText(b.firstChild,quayLine(0,i));
    setText(b.lastChild,i===visitor?'On the quay':'On rounds');
    b.setAttribute('aria-pressed',String(i===p));
  }
  const portrait=p+'/'+watch;
  if(portrait!==quayDesk.portrait){paintNeighbor('quay-portrait',p);quayDesk.portrait=portrait}
  setText($('quay-role'),quayLine(1,p));setText($('quay-name'),quayLine(0,p));
  setText($('quay-place'),quayLine(2,p)+' · '+quayLine(4,p));
  setText($('quay-greeting'),'“'+greeting+'”');
  const relation=visits+' scheduled '+(visits===1?'call':'calls')+' · '+helped+' '+(helped===1?'favor':'favors')+' done';
  setText($('quay-relation'),relation);
  const show=isHere&&quayDesk.asking;
  $('quay-favor').hidden=!show;
  setText($('quay-ask'),isHere?(show?'Fold up request':served?'Read the receipt':'Ask about work'):'Visit '+visitorName+' on the quay');
  $('quay-ask').setAttribute('aria-expanded',String(show));
  const canGive=Boolean(wasm.quay_can_help());
  if(show){
    const required=[],owned=[],missing=[];
    const sources=['a rowboat trip','a crab-pot haul','a ferry crossing','the cannery','the chandler'];
    quantities.forEach((n,g)=>{
      if(!n)return;
      required.push(n+' '+wireGoods[g].toLowerCase());
      owned.push(stock[g]+' '+wireGoods[g].toLowerCase());
      if(stock[g]<n)missing.push(wireGoods[g].toLowerCase()+' from '+sources[g]);
    });
    $('quay-departure').hidden=served;
    setText($('quay-request'),served?'Delivery received':'A favor for '+visitorName);
    setText($('quay-task'),'“'+quayLine(served?6:5,p,tier)+'”');
    setText($('quay-terms'),(served?'Delivered ':'Deliver ')+required.join(' + ')+' for '+reward+' coins.');
    setText($('quay-stock'),served?'Another neighbor will stop by next tide.':'In your hold: '+owned.join(' · ')+'.'+(missing.length?' Get '+missing.join(' and ')+'.':''));
    setText($('quay-give'),served?'Paid in full':canGive?'Deliver for '+reward+' coins':'Waiting for cargo');
    $('quay-give').disabled=served||!canGive;
  }
  quayDesk.shown=show&&!served?{tide,person:p,tier,reward,lanterns}:null;
  const memoryKey=p+'/'+helped;
  if(quayDesk.memories!==memoryKey){
    const list=$('quay-keepsakes'),n=Math.min(3,helped);
    list.textContent='';
    setText($('quay-memory').querySelector('summary'),'Keepsakes · '+n+' of 3');
    if(!n){
      const empty=document.createElement('p');empty.textContent='Help '+quayLine(0,p)+' on the quay to receive a keepsake.';list.appendChild(empty);
    }
    for(let i=0;i<n;i++){
      const h=document.createElement('h4'),body=document.createElement('p');
      h.textContent=quayLine(7,p,i);body.textContent=quayLine(8,p,i);list.append(h,body);
    }
    quayDesk.memories=memoryKey;
  }
  setText($('quay-line-a'),quayLine(9));setText($('quay-line-b'),quayLine(10));
  $('quay-lanterns').hidden=!lanterns;
  if(lanterns)setText($('quay-lanterns'),sailing?'The lantern flotilla is on the water. Look out past the end of the pier.':'At dusk, the neighbors sail a lantern flotilla past the pier.');
}

function clearQuayBell(){
  wasm.quay_bell_clear();quayAnswerUntil=0;
  $('hint').style.visibility='';
  setText($('quay-bell-answer'),'');
}
function pollQuayBell(now){
  const person=wasm.quay_bell_poll(now);
  if(person<3){
    setText($('quay-bell-answer'),quayLine(13,person));
    $('hint').style.visibility='hidden';
    quayAnswerUntil=now+4200;
    playHarborReply(person);
  }else if(quayAnswerUntil&&now>=quayAnswerUntil){
    quayAnswerUntil=0;$('hint').style.visibility='';
    setText($('quay-bell-answer'),'');
  }
}
