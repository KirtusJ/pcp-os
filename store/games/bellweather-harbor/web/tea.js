// Each line is a shared immutable Rust string. Position in the order paper and
// the tin's money belong to the save, so reopening the page resumes the sitting.
const teaCopy=new Map();
const teaStages=['Invitation','Tea Service','Quorum','High Tea','Adjourned'];
const teaSceneNames=['An envelope by the pier','The tea table on the pier','The committee takes over the slipway','A teapot and floating chairs occupy the harbor','A souvenir on the pier'];
let teaExpected=-1,teaRendered=-1,teaArchiveKey='',teaSceneStage=-1;
let teaBarterQuote=null,teaBarterTide=null;
function teaLine(field,index=0){
  const key=field+'/'+index;
  if(!teaCopy.has(key)){
    const p=wasm.tea_text_ptr(field,index),n=wasm.tea_text_len(field,index);
    teaCopy.set(key,quayDecoder.decode(new Uint8Array(wasm.memory.buffer,p,n)));
  }
  return teaCopy.get(key);
}
function paintTeaWindow(){
  if(!$('tea-book').open)return;
  const s=wasm.tea_stage(),canvas=$('tea-view'),context=canvas.getContext('2d',{alpha:false});
  // Early proceedings all fit on the left pier. During Quorum the committee
  // takes over the width of the harbor and the crane becomes part of the set.
  const wide=s===2||s===3;
  const source=wide?[0,51,448,131]:[0,102,217,71];
  if(teaSceneStage!==s){
    canvas.width=source[2];canvas.height=source[3];
    canvas.setAttribute('aria-label',teaSceneNames[s]);teaSceneStage=s;
  }
  context.imageSmoothingEnabled=false;
  context.drawImage($('harbor'),...source,0,0,canvas.width,canvas.height);
}
function teaReceipt(message){setText($('tea-receipt'),message)}
function closeTea(){
  if($('tea-book').open)$('tea-book').close();
  ($('tea-open').hidden?$('quay-open'):$('tea-open')).focus();
}
function initTea(){
  const desk=$('tea-book');
  $('tea-open').onclick=()=>{
    if(!wasm)return;
    actStart();
    if(!wasm.tea_invited()&&!wasm.tea_stage()){renderTea();return}
    desk.showModal();teaReceipt('');teaRendered=-1;teaSceneStage=-1;teaBarterTide=null;
    $('tea-committee').open=false;$('tea-record').open=false;
    renderTea();draw(time(),true);desk.scrollTop=0;$('tea-close').focus();
  };
  $('tea-close').onclick=closeTea;
  desk.addEventListener('cancel',e=>{e.preventDefault();closeTea()});
  desk.addEventListener('keydown',e=>{
    if(e.key==='Escape'&&desk.open){e.preventDefault();e.stopPropagation();closeTea()}
  });
  desk.addEventListener('click',e=>{
    if(e.target!==desk)return;
    const r=desk.getBoundingClientRect();
    if(e.clientX<r.left||e.clientX>r.right||e.clientY<r.top||e.clientY>r.bottom)closeTea();
  });
  $('tea-sign').onclick=()=>{
    actStart();
    if(wasm.tea_sign()){
      finishAction();sound([440,554,659]);teaReceipt('Signed. The secretary opens the order paper.');
      focusTeaAction();
    }else renderTea();
  };
  $('tea-motion').onclick=e=>{
    if(e.detail>1)return;
    const seen=teaExpected;
    actStart();
    if(seen<0||!wasm.tea_submit(seen)){
      renderTea();teaReceipt('This motion could not be filed. Check the current order paper and your hold.');return;
    }
    finishAction();
    teaReceipt(teaLine(9,seen));
    sound(seen===2||seen===5?[392,494,587,784]:[440,554]);
    focusTeaAction();
  };
  $('tea-claim').onclick=()=>{
    actStart();const amount=wasm.tea_claim();
    finishAction();
    teaReceipt(amount?'Moved '+amount.toLocaleString()+' coins from the tin to your till.':'The tin is empty.');
    focusTeaAction();
  };
  $('tea-barter-trade').onclick=()=>{
    const seen=teaBarterQuote;
    if(!seen)return;
    actStart();
    const settled=wasm.tea_barter(seen.lo,seen.hi);
    finishAction();
    const message=settled?'Traded '+seen.give+' for '+seen.take+'. The sea pockets the receipt.':
      (wasm.tea_tide_lo()!==seen.lo||wasm.tea_tide_hi()!==seen.hi)?'The tide changed. Read the new offer before trading.':'Your cargo or available space changed. Check the ticket again.';
    setText($('tea-barter-result'),message);
    if(settled)sound([330,440,659]);
  };
  $('tea-keep').onclick=()=>{
    actStart();renderTea();
    if(wasm.tea_stage()===3){teaReceipt(teaLine(2));closeTea();note(teaLine(2))}
  };
  $('tea-adjourn').onclick=()=>{
    actStart();const tin=wasm.tea_tin();
    if(!wasm.tea_adjourn()){renderTea();return}
    finishAction();sound([392,494,587,784]);
    teaReceipt('Adjourned. The till received '+tin.toLocaleString()+' coins from the tin and 13 attendance coins.');
    $('tea-close').focus();
  };
}
function focusTeaAction(){
  if(wasm.tea_stage()===3)$('tea-keep').focus();
  else if(!$('tea-motion').disabled)$('tea-motion').focus();
  else $('tea-close').focus();
}
function teaArchive(stage,motions){
  const key=stage+'/'+motions;
  if(key===teaArchiveKey)return;
  const grannies=$('tea-grannies'),filed=$('tea-filed');
  if(!grannies.childElementCount){
    for(let i=0;i<3;i++){
      const h=document.createElement('h4'),r=document.createElement('small'),p=document.createElement('p');
      h.textContent=teaLine(4,i);r.textContent=teaLine(5,i);h.appendChild(r);p.textContent='“'+teaLine(6,i)+'”';
      grannies.append(h,p);
    }
  }
  filed.textContent='';
  for(let i=0;i<motions;i++){
    const e=document.createElement('li'),b=document.createElement('strong'),p=document.createElement('span');
    b.textContent=teaLine(7,i);p.textContent=teaLine(9,i);e.append(b,p);filed.appendChild(e);
  }
  teaArchiveKey=key;
}
function renderTea(){
  const stage=wasm.tea_stage(),invited=Boolean(wasm.tea_invited()),active=stage>=1&&stage<=3;
  $('tea-open').hidden=!stage&&!invited;
  setText($('income-label'),active?'Gross fare rate':'Automatic income');
  if(!stage&&!invited){
    if($('tea-book').open)closeTea();
    return;
  }
  setText($('tea-entry'),stage===0?'An envelope under the sugar bowl':stage===4?'The committee minute book':'Emergency Tea Committee');
  setText($('tea-entry-action'),stage===0?'Read ↗':stage===4?'Open ↗':teaStages[stage]+' ↗');
  if(!$('tea-book').open)return;
  const motions=wasm.tea_motions(),tin=wasm.tea_tin(),good=wasm.tea_good();
  if(stage!==teaRendered){
    $('tea-invitation').hidden=stage!==0;$('tea-active').hidden=!active;
    $('tea-agenda').hidden=stage!==1&&stage!==2;$('tea-decision').hidden=stage!==3;
    $('tea-souvenir').hidden=stage!==4;
    $('tea-barter').hidden=stage!==3;
    setText($('tea-stage'),teaStages[stage]);
    setText($('tea-treaty'),teaLine(0));setText($('tea-programme'),teaLine(1));
    setText($('tea-chair-quote'),'“'+teaLine(6,0)+'” — '+teaLine(4,0)+', '+teaLine(5,0));
    setText($('tea-keep-text'),teaLine(2));setText($('tea-adjourn-text'),teaLine(3));
    setText($('tea-souvenir-title'),teaLine(11));setText($('tea-souvenir-text'),teaLine(12));
    teaRendered=stage;
  }
  setText($('tea-count'),stage===0?'Awaiting signature':motions+' / 6 motions filed');
  setText($('tea-tin'),tin.toLocaleString());$('tea-claim').disabled=tin<=0;
  setText($('tea-claim'),tin?'Collect '+tin.toLocaleString()+' coins':'Tin empty');
  teaExpected=-1;
  if((stage===1||stage===2)&&motions<6&&good<5){
    teaExpected=motions;
    const item=wireGoods[good].toLowerCase(),held=wasm.market_held(good),person=wasm.tea_sponsor(motions);
    setText($('tea-agenda-label'),'Motion '+(motions+1)+' · '+teaLine(4,person));
    setText($('tea-proposal-title'),teaLine(7,motions));
    setText($('tea-proposal'),'“'+teaLine(8,motions)+'”');
    const source=good===0?'Send out the rowboat for salt.':'The cannery makes provisions from salt and crab.';
    setText($('tea-budget'),'Needs 1 '+item+' crate. You have '+held+'.'+(held?'':' '+source));
    setText($('tea-motion'),'File with 1 '+item);$('tea-motion').disabled=!wasm.tea_can_submit();
  }
  if(stage===3)renderSeaBarter();else teaBarterQuote=null;
  teaArchive(stage,motions);
  // The committee's real-time minutes stop at adjournment. The filed motions
  // stay available in the book once the last attendance coin has been paid.
  setText($('tea-minute'),stage===4?'The roll is closed. All six motions are on file.':teaLine(10,wasm.freight_tide()%12));
}

function renderSeaBarter(){
  const lo=wasm.tea_tide_lo(),hi=wasm.tea_tide_hi(),key=hi+'/'+lo;
  const send=wasm.tea_barter_good(0),get=wasm.tea_barter_good(1);
  if(send>=wireGoods.length||get>=wireGoods.length)return;
  const sendQty=wasm.tea_barter_qty(0),getQty=wasm.tea_barter_qty(1);
  const a=wireGoods[send].toLowerCase(),b=wireGoods[get].toLowerCase();
  const give=sendQty+' '+a,take=getQty+' '+b;
  const held=wasm.market_held(send),stock=wasm.market_held(get),room=wasm.tea_barter_room(get);
  const reserved=room+stock<wasm.freight_capacity(),done=wasm.tea_barter_done();
  if(teaBarterTide!==key){teaBarterTide=key;setText($('tea-barter-result'),'')}
  teaBarterQuote={lo,hi,give,take};
  setText($('tea-barter-tide'),Math.ceil(wasm.freight_tide_ms()/1000)+'s until next tide');
  setText($('tea-barter-terms'),'Give '+give+' · Receive '+take);
  setText($('tea-barter-hold'),'Aboard: '+held+' '+a+' and '+stock+' '+b+'. Space for '+room+' more '+b+'.'+(reserved?' One space is held for the current workshop batch.':''));
  const reasons=[];
  if(held<sendQty)reasons.push('Need '+(sendQty-held)+' more '+a+'.');
  if(room<getQty)reasons.push('Free '+(getQty-room)+' more '+b+' space'+(getQty-room===1?'':'s')+'.');
  setText($('tea-barter-status'),done?'This tide’s swap is complete. A new offer arrives next tide.':reasons.length?reasons.join(' '):'One swap per tide while High Tea sits.');
  setText($('tea-barter-trade'),done?'Swap complete':'Trade cargo');
  $('tea-barter-trade').disabled=done||!wasm.tea_can_barter();
}
