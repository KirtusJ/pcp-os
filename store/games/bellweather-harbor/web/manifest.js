// A receipt from the harbor desk. Read marks, findings and the final resolution
// live in the Rust ledger. The document currently on the desk and the two pins
// are a local working selection until the evidence is filed.
const manifestDecoder=new TextDecoder('utf-8');
const manifestCopy=new Map();
const manuscript={chapter:0,lead:0,pins:[],feedback:'',choice:0,signature:'',picture:''};
const manifestText=(field,ch,i=0)=>{
  const key=field+'/'+ch+'/'+i;
  if(!manifestCopy.has(key)){
    const length=wasm.manifest_text_len(field,ch,i),ptr=wasm.manifest_text_ptr(field,ch,i);
    manifestCopy.set(key,manifestDecoder.decode(new Uint8Array(wasm.memory.buffer,ptr,length)));
  }
  return manifestCopy.get(key);
};
function caseButton(id){
  const b=document.createElement('button');
  b.type='button';b.id=id;
  return b;
}
function clearManifestDraft(){
  manuscript.pins=[];manuscript.feedback='';manuscript.choice=0;manuscript.signature='';
}
function focusManifestStart(){
  const book=$('manifest-book');
  book.scrollTop=0;
}
function browseManifest(ch,lead=0){
  if(!wasm||!wasm.manifest_unlocked()||ch<0||ch>Math.min(3,wasm.manifest_solved()))return;
  actStart();
  if(ch>Math.min(3,wasm.manifest_solved()))return;
  if(ch!==3)stopWireSignal();
  clearManifestDraft();manuscript.chapter=ch;manuscript.lead=lead;
  if(ch<3 && !(wasm.manifest_read(ch)&(1<<lead)))wasm.manifest_inspect(ch,lead);
  finishAction();focusManifestStart();
}
function inspectManifest(i){
  actStart();
  if(manuscript.chapter>=3||manuscript.chapter>wasm.manifest_solved())return;
  manuscript.lead=i;wasm.manifest_inspect(manuscript.chapter,i);finishAction();
}
function initManifest(){
  const book=$('manifest-book');
  initWire();
  $('manifest-open').onclick=()=>{
    if(!wasm||!wasm.manifest_unlocked())return;
    actStart();
    const solved=wasm.manifest_solved(), ch=Math.min(3,solved);
    book.showModal();browseManifest(ch);
    $('manifest-close').focus();
  };
  function closeDesk(){
    if(book.open)book.close();
    const receipt=$('manifest-open');
    if(!receipt.hidden)receipt.focus();
  }
  $('manifest-close').onclick=closeDesk;
  book.addEventListener('keydown',e=>{
    if(e.key==='Escape' && book.open){
      e.preventDefault();e.stopPropagation();closeDesk();
    }
  });
  book.addEventListener('cancel',e=>{e.preventDefault();closeDesk();});
  book.addEventListener('close',()=>{
    clearManifestDraft();manuscript.picture='';
    const receipt=$('manifest-open');
    if(!book.open&&!receipt.hidden)receipt.focus();
  });
  book.addEventListener('click',e=>{
    if(e.target!==book)return;
    const r=book.getBoundingClientRect();
    if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)closeDesk();
  });
  for(let i=0;i<4;i++){
    const tab=caseButton('manifest-chapter-'+i);
    tab.onclick=()=>browseManifest(i);
    $('manifest-chapters').appendChild(tab);
    const lead=caseButton('manifest-lead-'+i);
    lead.appendChild(document.createElement('b'));lead.appendChild(document.createElement('small'));
    lead.onclick=()=>inspectManifest(i);$('manifest-leads').appendChild(lead);
  }
  for(let choice=1;choice<=2;choice++){
    const b=caseButton('manifest-ending-'+choice);
    b.appendChild(document.createElement('strong'));b.appendChild(document.createElement('span'));
    b.onclick=()=>{manuscript.choice=choice;renderManifest();$('manifest-confirm').focus()};
    $('manifest-choices').appendChild(b);
  }
  $('manifest-pin').onclick=()=>{
    const i=manuscript.lead,pos=manuscript.pins.indexOf(i);
    if(pos>=0)manuscript.pins.splice(pos,1);
    else if(manuscript.pins.length<2)manuscript.pins.push(i);
    manuscript.feedback='';renderManifest();
  };
  $('manifest-submit').onclick=()=>{
    if(manuscript.pins.length!==2)return;
    actStart();
    const ch=manuscript.chapter;
    if(ch!==wasm.manifest_solved()){renderManifest();return}
    if(wasm.manifest_prove(ch,manuscript.pins[0],manuscript.pins[1])){
      manuscript.feedback='';manuscript.pins=[];sound([392,494,587]);finishAction();$('manifest-next').focus();
    }else{
      manuscript.feedback=manifestText(3,ch);
      finishAction();
    }
  };
  $('manifest-next').onclick=()=>{browseManifest(manuscript.chapter+1);$('manifest-chapter-'+manuscript.chapter).focus()};
  $('manifest-ending-cancel').onclick=()=>{
    const c=manuscript.choice;manuscript.choice=0;renderManifest();$('manifest-ending-'+c).focus();
  };
  $('manifest-confirm').onclick=()=>{
    actStart();const c=manuscript.choice;
    if(c&&wasm.manifest_choose(c)){sound(c===1?[392,523,659]:[349,440,523]);manuscript.choice=0;finishAction();$('manifest-close').focus()}
    else renderManifest();
  };
}
function renderPins(){
  const holder=$('manifest-pins'), ch=manuscript.chapter;
  holder.textContent='';
  manuscript.pins.forEach(i=>{
    const b=caseButton('manifest-unpin-'+i);
    b.textContent=manifestText(5,ch,i)+' ×';
    b.setAttribute('aria-label','Unpin '+manifestText(5,ch,i));
    b.onclick=()=>{manuscript.pins=manuscript.pins.filter(v=>v!==i);manuscript.feedback='';renderManifest();$('manifest-pin').focus()};
    holder.appendChild(b);
  });
  if(!manuscript.pins.length){const e=document.createElement('span');e.textContent='No documents pinned';holder.appendChild(e)}
}
function renderManifest(){
  const unlocked=!!wasm.manifest_unlocked(),solved=wasm.manifest_solved(),ending=wasm.manifest_ending();
  const read=wasm.manifest_read(0),tip=wasm.bell_tip();
  setText($('bell-pay'),'Earn '+tip+' coin'+(tip===1?'':'s')+' and send the rowboat out.');
  $('manifest-open').hidden=!unlocked;
  const opened=read||solved||ending;
  setText($('manifest-entry'),opened?'The Second Manifest':'An odd receipt');
  setText($('manifest-entry-action'),ending?'Reopen file ↗':opened?'Read file ↗':'Examine ↗');
  if(!$('manifest-book').open)return;
  if(!unlocked){$('manifest-book').close();return}
  if(manuscript.chapter>solved){clearManifestDraft();manuscript.chapter=solved;manuscript.lead=0}
  const ch=manuscript.chapter, i=manuscript.lead, decision=ch===3, confirmed=ch<solved, marks=decision?0:wasm.manifest_read(ch);
  // The telegram and its live book have a different clock from the case pages.
  if(decision)renderWire(ending);
  const key=[ch,i,solved,ending,marks,manuscript.pins,manuscript.feedback,manuscript.choice].join('|');
  if(key===manuscript.signature)return;
  manuscript.signature=key;
  for(let j=0;j<4;j++){
    const tab=$('manifest-chapter-'+j);
    setText(tab,j===3?'Disposition':'File '+(j+1)+(j<solved?' ✓':''));
    tab.title=j===3?'Final disposition':j>solved?'File is sealed':manifestText(0,j);
    tab.setAttribute('aria-label',tab.textContent+(j<3&&j<=solved?' · '+manifestText(0,j):''));
    tab.disabled=j>solved;tab.setAttribute('aria-pressed',String(ch===j));
  }
  setText($('manifest-folio'),decision?'The harbor register':'Harbor records · File '+(ch+1)+' of 3');
  $('manifest-desk').hidden=decision;$('manifest-findings').hidden=decision||confirmed;
  $('manifest-verdict').hidden=decision||!confirmed;$('manifest-decision').hidden=!decision;
  if(decision){renderDisposition(ending);return}
  setText($('manifest-heading'),manifestText(0,ch));setText($('manifest-scene'),manifestText(1,ch));
  for(let j=0;j<4;j++){
    const lead=$('manifest-lead-'+j),pinned=manuscript.pins.includes(j);
    const filed=confirmed && !!(wasm.manifest_evidence(ch)&(1<<j));
    setText(lead.firstChild,manifestText(5,ch,j));setText(lead.lastChild,filed?'Filed evidence':pinned?'Pinned':marks&(1<<j)?'Read':'Unread');
    lead.setAttribute('aria-pressed',String(i===j));
  }
  setText($('manifest-source'),manifestText(6,ch,i));setText($('manifest-doc-title'),manifestText(5,ch,i));setText($('manifest-body'),manifestText(7,ch,i));
  const artKey=ch+'/'+i+'/'+ending;
  if(artKey!==manuscript.picture){
    drawManifestArt($('manifest-art'),ch,i,ending);
    $('manifest-art').setAttribute('aria-label',['The quay scale at dusk','Ada’s communal kitchen','The harbor press and telegraph'][ch]+', with '+manifestText(5,ch,i).toLowerCase()+' in close-up.');
    manuscript.picture=artKey;
  }
  $('manifest-pin').hidden=confirmed;
  if(confirmed){
    setText($('manifest-reveal'),manifestText(4,ch));
    const proof=wasm.manifest_evidence(ch);
    const citations=[0,1,2,3].filter(v=>proof&(1<<v)).map(v=>manifestText(5,ch,v));
    setText($('manifest-citation'),'Filed with '+citations.join(' and ')+'.');
    setText($('manifest-next'),ch===2?'Read the disposition →':'Continue to file '+(ch+2)+' →');
    return;
  }
  const pinned=manuscript.pins.includes(i);
  $('manifest-pin').disabled=!pinned&&manuscript.pins.length===2;
  $('manifest-pin').setAttribute('aria-pressed',String(pinned));
  setText($('manifest-pin'),pinned?'Unpin this document':'Pin this document');
  setText($('manifest-question'),manifestText(2,ch));renderPins();
  $('manifest-submit').disabled=manuscript.pins.length!==2;
  setText($('manifest-instructions'),manuscript.pins.length===2?'File the pair or remove a document to change it.':'Pin two documents that answer the question.');
  setText($('manifest-feedback'),manuscript.feedback);
}
function renderDisposition(ending){
  const choice=manuscript.choice,final=ending>0;
  const picture='ending/'+ending;
  if(picture!==manuscript.picture){
    drawManifestArt($('manifest-ending-art'),ending===2?1:2,-1,ending);
    $('manifest-ending-art').setAttribute('aria-label',ending===2?'The night kitchen, with a row of mugs ready for the crew.':ending===1?'The public press, with the first sheets on its bench.':'The harbor desk at the end of the investigation.');
    manuscript.picture=picture;
  }
  setText($('manifest-heading'),final?'The decision on file':'What goes on the record?');
  setText($('manifest-scene'),final?'The case files remain available in the register.':'Choose what the harbor will do with the second manifest. The decision stays with this harbor through every charter.');
  $('manifest-choices').hidden=final||!!choice;
  $('manifest-confirmation').hidden=final||!choice;
  $('manifest-epilogue').hidden=!final;
  for(let j=0;j<2;j++){
    const b=$('manifest-ending-'+(j+1));
    setText(b.firstChild,manifestText(8,0,j));setText(b.lastChild,manifestText(9,0,j));
  }
  if(choice&&!final){
    setText($('manifest-decision-title'),manifestText(8,0,choice-1)+'?');
    setText($('manifest-decision-description'),manifestText(9,0,choice-1)+' This is the final entry in the register.');
  }
  if(final){
    setText($('manifest-epilogue-title'),manifestText(8,0,ending-1));
    setText($('manifest-epilogue-text'),manifestText(10,0,ending-1));
    setText($('manifest-effect'),manifestText(9,0,ending-1));
  }
}


const wireState={tide:null,ending:0,replied:-1,failed:-1,signature:'',changed:false};
function resetWire(){
  stopWireSignal();
  wireState.tide=null;wireState.ending=0;wireState.replied=-1;wireState.failed=-1;
  wireState.signature='';wireState.changed=false;
}
const wireGoods=['Salt','Crab','Timber','Provisions','Fittings'];
let wireTones=[];
function stopWireSignal(){
  wireTones.forEach(o=>{try{o.stop()}catch(_){}});
  wireTones=[];
}
async function replayWire(){
  if(!sounding||!wasm||!wasm.manifest_ending())return;
  actStart();
  const tide=wasm.freight_tide(),groups=[wasm.wire_group(0),wasm.wire_group(1)];
  stopWireSignal();
  try{
    const C=window.AudioContext||window.webkitAudioContext;
    if(!C){setText($('wire-feedback'),'Audio is unavailable in this browser. The call is printed above.');return}
    if(!audioContext)audioContext=new C();
    const ac=audioContext;
    if(ac.state==='suspended')await ac.resume();
    if(!sounding||!$('manifest-book').open||wasm.freight_tide()!==tide)return;
    const unit=.09;let at=ac.currentTime+.02;
    groups.forEach((pattern,group)=>{
      if(group)at+=unit*2; // One unit was already left after the third pulse.
      for(let bit=2;bit>=0;bit--){
        const length=(pattern&(1<<bit)?3:1)*unit;
        const oscillator=ac.createOscillator(),gain=ac.createGain();
        oscillator.type='sine';oscillator.frequency.value=680;
        gain.gain.setValueAtTime(0,at);gain.gain.linearRampToValueAtTime(.11,at+.006);
        gain.gain.setValueAtTime(.11,at+length-.006);gain.gain.linearRampToValueAtTime(0,at+length);
        oscillator.connect(gain);gain.connect(ac.destination);oscillator.start(at);oscillator.stop(at+length+.001);
        oscillator.onended=()=>{oscillator.disconnect();gain.disconnect();wireTones=wireTones.filter(o=>o!==oscillator)};
        wireTones.push(oscillator);at+=length+unit;
      }
    });
  }catch(_){setText($('wire-feedback'),'Audio could not play. You can still read the printed call.')}
}
function signalString(code,spoken=false){
  const words=[];
  for(let bit=2;bit>=0;bit--)words.push(code&(1<<bit)?(spoken?'dash':'–'):(spoken?'dot':'•'));
  return words.join(spoken?' ':'');
}
function signalPair(first,second,spoken=false){
  return signalString(first,spoken)+(spoken?', then ':'   /   ')+signalString(second,spoken);
}
function initWire(){
  $('wire-replay').onclick=replayWire;
  $('manifest-book').addEventListener('close',stopWireSignal);
  wireGoods.forEach((good,i)=>{
    const b=caseButton('wire-guess-'+i);
    const name=document.createElement('strong'),code=document.createElement('span');
    name.textContent=good;b.append(name,code);$('wire-directory').appendChild(b);
    b.onclick=()=>{
      const seen=wireState.tide;
      actStart();
      if(!wasm.manifest_ending()||seen!==wasm.freight_tide()){
        renderWire(wasm.manifest_ending());return;
      }
      if(wasm.wire_decode(i)){
        wireState.replied=i;wireState.failed=-1;sound([440,587]);
      }else{
        wireState.replied=-1;wireState.failed=i;sound([293.7]);
      }
      renderWire(wasm.manifest_ending());
    };
  });
  ['Now','Next tide','In 2 tides','In 3 tides'].forEach((label,i)=>{
    const slot=document.createElement('div'),tag=document.createElement('span'),amount=document.createElement('b');
    tag.textContent=label;amount.id='wire-bid-'+i;slot.append(tag,amount);$('wire-quotes').appendChild(slot);
  });
}
function renderWire(ending){
  const panel=$('wire-panel');panel.hidden=!ending;
  if(!ending){resetWire();return}
  if(ending!==wireState.ending){resetWire();wireState.ending=ending}
  const tide=wasm.freight_tide();
  if(tide!==wireState.tide){
    stopWireSignal();
    wireState.changed=wireState.tide!==null;
    wireState.tide=tide;wireState.replied=-1;wireState.failed=-1;wireState.signature='';
  }
  const left=Math.ceil(wasm.freight_tide_ms()/1000),replied=wireState.replied;
  const bids=replied>=0?[0,1,2,3].map(k=>wasm.wire_bid(replied,k)):[];
  const signature=[tide,left,replied,wireState.failed,bids,sounding].join('|');
  if(signature===wireState.signature)return;
  wireState.signature=signature;
  const replay=$('wire-replay');
  replay.disabled=!sounding;
  setText(replay,sounding?'Replay signal':'Replay · sound off');
  replay.title=sounding?'Play this call as short and long tones':'Enable Sound in the harbor header to hear the call.';
  setText($('wire-tide'),'Tide '+(tide+1)+' · '+left+'s');
  const first=wasm.wire_group(0),second=wasm.wire_group(1);
  setText($('wire-groups'),signalPair(first,second));
  $('wire-groups').setAttribute('aria-label',signalPair(first,second,true));
  wireGoods.forEach((good,i)=>{
    const b=$('wire-guess-'+i),a=wasm.wire_call(i,0),z=wasm.wire_call(i,1);
    setText(b.lastChild,signalPair(a,z));b.setAttribute('aria-label',good+': '+signalPair(a,z,true));
    b.disabled=replied>=0;b.classList.toggle('answered',i===replied);
  });
  $('wire-report').hidden=replied<0;
  let feedback='';
  if(replied>=0){
    feedback='Matched the '+wireGoods[replied]+' desk.';
    setText($('wire-report-title'),wireGoods[replied]+' · one-crate bids');
    bids.forEach((v,k)=>setText($('wire-bid-'+k),v>0?num(v):'No bid'));
  }else if(wireState.failed>=0){
    const g=wireState.failed,matchFirst=wasm.wire_call(g,0)===first,matchSecond=wasm.wire_call(g,1)===second;
    const problem=matchFirst?'The first group matches. Check the second.':matchSecond?'The second group matches. Check the first.':'Compare each group in order.';
    feedback=wireGoods[g]+' uses a different call. '+problem+' Try another desk.';
  }else if(wireState.changed)feedback='A new call has arrived.';
  setText($('wire-feedback'),feedback);
}
