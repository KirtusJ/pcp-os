// One standing sale instruction can remain with the broker through twelve clearings.
const brokerStatuses = ['Empty', 'Pending', 'Filled', 'Expired', 'Cancelled'];
let brokerDraftGood = 0, brokerDidEditFloor = false, brokerNotice = '', brokerNoticeUntil = 0;
let brokerQuoteKey = '', brokerQuoteText = '', brokerLastStatus = '';

function brokerMessage(text) {
  brokerNotice = text;
  brokerNoticeUntil = time() + 7000;
}
function brokerReadNumber(id, min, max) {
  const raw = $(id).value.trim(), number = Number(raw);
  return raw !== '' && Number.isInteger(number) && number >= min && number <= max ? number : null;
}
function initBroker() {
  $('broker-panel').innerHTML = `<div class="broker-book">
    <header class="broker-intro"><div><h3>Harbor broker</h3><p>Leave instructions to sell cargo as it comes ashore.</p></div>
      <canvas id="broker-seal" width="40" height="32" aria-hidden="true"></canvas></header>
    <div class="broker-grid">
      <section class="broker-entry" aria-labelledby="broker-entry-head">
        <h4 id="broker-entry-head">Standing sale</h4>
        <form id="broker-form" novalidate>
          <label for="broker-good">Cargo</label><select id="broker-good"></select>
          <div class="broker-fields">
            <label for="broker-qty">Quantity <small>crates</small><input id="broker-qty" type="number" min="1" max="96" step="1" required value="1" inputmode="numeric" autocomplete="off"></label>
            <label for="broker-floor">Minimum per crate <small>coins</small><input id="broker-floor" type="number" min="1" max="65535" step="1" required inputmode="numeric" autocomplete="off"></label>
          </div>
          <p id="broker-range" class="broker-hint">1–96 crates · 1–65,535 coins per crate</p>
          <button id="broker-place" class="broker-primary" type="submit">File standing sale</button>
        </form>
        <p id="broker-policy" class="broker-help">At each of the next 12 tide clearings, the broker sells available stock while each crate earns your minimum. You can trade the stock in the meantime.</p>
      </section>
      <section class="broker-order" aria-labelledby="broker-status-head">
        <div class="broker-headline"><h4 id="broker-status-head">Broker’s copy</h4><strong id="broker-status">Empty</strong></div>
        <p id="broker-instruction">No instruction filed.</p>
        <dl class="broker-lines">
          <div><dt>Requested</dt><dd id="broker-requested">—</dd></div>
          <div><dt>Filled</dt><dd id="broker-filled">—</dd></div>
          <div><dt>Unfilled</dt><dd id="broker-remaining">—</dd></div>
          <div><dt>Tides left</dt><dd id="broker-tides">—</dd></div>
          <div class="broker-paid"><dt>Proceeds received</dt><dd id="broker-proceeds">—</dd></div>
        </dl>
        <button id="broker-cancel" type="button" hidden>Cancel remaining sale</button>
      </section>
    </div>
    <section class="broker-preview" aria-labelledby="broker-preview-head">
      <div class="broker-market"><div><h4 id="broker-preview-head">Current order book</h4><span id="broker-position"></span></div>
        <div class="broker-bid"><small>First-crate bid</small><strong id="broker-current-bid">—</strong></div></div>
      <p id="broker-preview"></p>
      <p class="broker-help">The broker checks fresh bids and stock at each clearing.</p>
    </section>
    <p id="broker-receipt" class="broker-receipt" role="status"></p>
  </div>`;
  paintCargo($('broker-seal'),3);
  const good=$('broker-good');
  worksGoods.forEach((name,g)=>{
    const option=document.createElement('option');option.value=String(g);option.textContent=name;good.appendChild(option);
  });
  good.onchange=()=>{
    brokerDraftGood=Number(good.value);
    if(!brokerDidEditFloor&&wasm) $('broker-floor').value=String(Math.max(1,wasm.market_bid(brokerDraftGood)));
    renderBroker();
  };
  $('broker-floor').addEventListener('input',()=>{brokerDidEditFloor=true;renderBroker();});
  $('broker-qty').addEventListener('input',renderBroker);
  $('broker-form').onsubmit=event=>{
    event.preventDefault();
    if(!wasm) return;
    const qty=brokerReadNumber('broker-qty',1,96),floor=brokerReadNumber('broker-floor',1,65535);
    if(!qty||!floor){renderBroker();return;}
    actStart();
    if(wasm.ticket_place(brokerDraftGood,qty,floor)){
      brokerMessage('Standing sale filed. The first check is at the next tide clearing.');
      sound([392,494,587]);
    } else brokerMessage('The broker could not file this instruction. Check the current copy before filing again.');
    finishAction();
  };
  $('broker-cancel').onclick=()=>{
    const id=$('broker-cancel').dataset.instruction;
    actStart();
    const active=wasm.ticket_status()===1;
    const matches=id===brokerIdentity();
    if(active&&matches&&wasm.ticket_cancel()){
      brokerMessage('Sale cancelled. All proceeds already received stay in your cash account.');
      sound([494,392]);
    } else brokerMessage('The standing sale changed. Review the broker’s current copy.');
    finishAction();
  };
}
function brokerIdentity() {
  return [wasm.ticket_good(),wasm.ticket_requested(),wasm.ticket_floor(),wasm.freight_tide()+wasm.ticket_tides_left()].join('/');
}
function renderBroker() {
  if(!wasm||!$('exchange-book').open||$('broker-panel').hidden) return;
  const status=wasm.ticket_status(), active=status===1;
  const requested=wasm.ticket_requested(), filled=wasm.ticket_filled(), remaining=wasm.ticket_remaining();
  const floor=wasm.ticket_floor(), good=wasm.ticket_good(), proceeds=wasm.ticket_proceeds(), tides=wasm.ticket_tides_left();
  const record=status>0&&good>=0&&good<5;
  const stamp=brokerStatuses[status]||'Empty';
  setText($('broker-status'),stamp);$('broker-status').className='broker-state-'+status;
  setText($('broker-instruction'),record?worksGoods[good]+' · minimum '+worksAmount(floor)+' coins per crate':'No instruction filed.');
  setText($('broker-requested'),record?worksAmount(requested):'—');
  setText($('broker-filled'),record?worksAmount(filled):'—');
  setText($('broker-remaining'),record?worksAmount(remaining):'—');
  setText($('broker-tides'),active?String(tides):record?'Closed':'—');
  setText($('broker-proceeds'),record?worksAmount(proceeds)+' coins':'—');
  const noteKey=[status,good,requested,filled,proceeds].join(',');
  if(brokerLastStatus!==noteKey){
    if(record&&filled>0&&brokerLastStatus){
      if(status===2) brokerMessage('Filled in full. The broker collected '+worksAmount(proceeds)+' coins.');
      else if(active) brokerMessage('Partial fill. '+worksAmount(filled)+' sold so far for '+worksAmount(proceeds)+' coins.');
    }
    if(status===3&&brokerLastStatus) brokerMessage('The instruction expired. '+worksAmount(remaining)+' remained unfilled.');
    brokerLastStatus=noteKey;
  }

  const inputGood=$('broker-good'), inputQty=$('broker-qty'), inputFloor=$('broker-floor');
  // Keep the live instruction together even after reload or an import.
  if(active) {
    inputGood.value=good;inputQty.value=requested;inputFloor.value=floor;
    brokerDraftGood=good;brokerDidEditFloor=true;
  } else if(!brokerDidEditFloor&&!inputFloor.value) {
    inputFloor.value=String(Math.max(1,wasm.market_bid(brokerDraftGood)));
  }
  inputGood.disabled=inputQty.disabled=inputFloor.disabled=active;
  const qty=brokerReadNumber('broker-qty',1,96), price=brokerReadNumber('broker-floor',1,65535);
  inputQty.setAttribute('aria-invalid',String(!qty));inputFloor.setAttribute('aria-invalid',String(!price));
  const place=$('broker-place'), cancel=$('broker-cancel');
  place.hidden=active;place.disabled=active||!qty||!price;cancel.hidden=!active;
  cancel.dataset.instruction=active?brokerIdentity():'';
  setText($('broker-entry-head'),active?'Filed instruction':'Standing sale');
  setText($('broker-range'),active?'This copy stays on file until filled, expired or cancelled.':
    !qty||!price?'Enter whole numbers: 1–96 crates and 1–65,535 coins per crate.':'1–96 crates · 1–65,535 coins per crate');

  const g=active?good:brokerDraftGood, draftN=active?remaining:qty, limit=active?floor:price;
  const stock=wasm.market_held(g), bid=wasm.market_bid(g),depth=wasm.market_stock(g);
  setText($('broker-current-bid'),bid?worksAmount(bid)+' coins':'No buyer');
  setText($('broker-position'),worksGoods[g]+' · '+worksAmount(stock)+' in your hold');
  // Merchant inventory and tide determine every step of the price curve.
  const key=[g,draftN,limit,stock,bid,depth,wasm.freight_tide()].join(',');
  if(key!==brokerQuoteKey){
    brokerQuoteKey=key;
    if(!draftN||!limit) brokerQuoteText='Set a valid quantity and minimum to price the instruction.';
    else if(stock===0) brokerQuoteText='No stock on hand. An instruction can wait for cargo you land, buy or make.';
    else {
      const count=wasm.ticket_preview_qty(g,draftN,limit);
      const cash=wasm.ticket_preview_total(g,draftN,limit);
      brokerQuoteText=count?'At current bids, '+worksAmount(count)+' of '+worksAmount(draftN)+(draftN===1?' crate':' crates')+
        (count===1?' meets':' meet')+' the minimum, for '+worksAmount(cash)+' coins.':'No available crate meets this minimum at current bids.';
    }
  }
  setText($('broker-preview'),brokerQuoteText);
  setText($('broker-receipt'),time()<brokerNoticeUntil?brokerNotice:active?'This instruction can fill while you are away.':
    record?'A new sale replaces the closed instruction in this book.':'Proceeds from broker sales appear in exchange accounts.');
}
