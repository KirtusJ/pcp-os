// The workshop floor reads its stock, quotes and settled accounts from the Rust game.
const worksNames = ['Cannery', 'Chandler’s Shop'];
const worksGoods = ['Salt', 'Crab', 'Timber', 'Provisions', 'Fittings'];
const worksRecipes = [
  'Pack the catch in salt and seal it for the crossing.',
  'Cut timber and salt-treat it for hard service at sea.'
];
let currentWorks = 0, worksSceneKey = '', worksNotice = '';
let worksNoticeUntil = 0;
const worksDigits = new Intl.NumberFormat('en', {maximumFractionDigits:0});
const worksAmount = value => worksDigits.format(Math.abs(value));
const worksSigned = value => (value < 0 ? '−' : value > 0 ? '+' : '') + worksAmount(value);

function worksMessage(message) {
  worksNotice = message;
  worksNoticeUntil = time() + 7000;
}
function initWorks() {
  const floor = $('works-panel'), journal = $('ledger-panel');
  floor.innerHTML = `
  <div class="works-book">
    <div class="works-switch" role="group" aria-label="Choose a workshop">
      <button type="button" id="works-kind-0" aria-pressed="true"><canvas aria-hidden="true"></canvas><span><strong>Cannery</strong><small>Provisions</small></span></button>
      <button type="button" id="works-kind-1" aria-pressed="false"><canvas aria-hidden="true"></canvas><span><strong>Chandler’s Shop</strong><small>Fittings</small></span></button>
    </div>
    <div class="works-portrait"><canvas id="works-art" width="360" height="96" role="img"></canvas></div>
    <div class="works-name"><h3 id="works-title"></h3><span id="works-time"></span></div>
    <div class="works-grid">
      <section class="works-deck" aria-labelledby="works-batch-heading">
        <h4 id="works-batch-heading">Batch ticket</h4>
        <ul id="works-recipe" class="works-recipe"></ul>
        <div class="works-clock"><span id="works-status"></span><strong id="works-clock"></strong></div>
        <div class="works-meter" id="works-meter" role="progressbar" aria-label="Batch progress" aria-valuemin="0" aria-valuemax="100"><span id="works-fill"></span></div>
        <div class="works-actions">
          <button class="works-primary" id="works-build" type="button"></button>
          <button class="works-primary" id="works-start" type="button"></button>
          <button id="works-repeat" type="button" aria-pressed="false"></button>
        </div>
        <p id="works-help" class="works-help"></p>
      </section>
      <section class="works-finance" aria-labelledby="works-value-heading">
        <h4 id="works-value-heading">Batch valuation</h4>
        <ul class="works-accounts">
          <li><span>Finished goods at current bids</span><strong id="works-output-value"></strong></li>
          <li><span>Inputs sold at current bids</span><strong id="works-input-value"></strong></li>
          <li><span>Processing fee</span><strong id="works-fee"></strong></li>
          <li class="works-gain"><span>Gain over selling inputs</span><strong id="works-next-margin"></strong></li>
        </ul>
        <div class="works-wholesale"><span>Gain if all inputs are bought</span><strong id="works-purchased-margin"></strong></div>
        <p id="works-value-help" class="works-help"></p>
        <div class="works-stockhead"><span id="works-stock-label"></span><strong id="works-stock"></strong></div>
        <div class="works-sales"><button id="works-sell-one" class="works-primary" type="button"></button><button id="works-sell-all" type="button"></button></div>
      </section>
    </div>
    <p id="works-receipt" role="status"></p>
  </div>`;
  const bookTable = (caption, rows, total) =>
    '<table class="ledger-table"><caption>' + caption + '</caption><tbody>' +
    rows.map(row=>'<tr><th scope="row">'+row[1]+'</th><td id="ledger-'+row[0]+'">0</td></tr>').join('') +
    '</tbody><tfoot><tr><th scope="row">Total</th><td id="ledger-'+total+'">0</td></tr></tfoot></table>';
  journal.innerHTML = `<div class="ledger-book">
    <div class="ledger-intro"><h3>Exchange accounts</h3><small>Current charter · coins</small></div>
    <div class="ledger-totals">
      <div><span>Receipts</span><strong id="ledger-receipts">0</strong></div>
      <div><span>Cash paid</span><strong id="ledger-paid">0</strong></div>
      <div><span>Net trading cash flow</span><strong id="ledger-net">0</strong></div>
    </div>
    <div class="ledger-postings">` +
    bookTable('Cash received', [['sales','Cargo sales'],['orders','Freight orders & quay favors']], 'receipts-total') +
    bookTable('Cash paid', [['purchases','Purchased raw cargo'],['fees','Processing fees'],['construction','Workshop openings']], 'paid-total') +
    `</div><section class="ledger-stock" aria-labelledby="ledger-stock-head">
      <h4 id="ledger-stock-head">Warehouse valuation</h4>
      <table class="ledger-table"><thead><tr><th scope="col">Cargo</th><th scope="col">Held</th><th scope="col">Buyer takes</th><th scope="col">Sale value</th></tr></thead>
        <tbody id="ledger-inventory"></tbody><tfoot><tr><th scope="row" colspan="3">Immediately saleable stock</th><td id="ledger-inventory-total">0</td></tr></tfoot>
      </table>
      <p id="ledger-depth" class="works-help"></p>
      <div class="ledger-liquid"><span>Cash + saleable stock</span><strong id="ledger-liquid">0</strong></div>
      <p class="works-help">Liquid assets use executable bids. The estimate excludes buildings, booked orders and work in progress.</p>
    </section>
    <details class="ledger-flows"><summary>Stock movements by cargo</summary>
      <div class="ledger-flow-select"><label for="ledger-flow-good">Cargo account</label><select id="ledger-flow-good"></select></div>
      <table class="ledger-table"><tbody id="ledger-flow-rows"></tbody>
        <tfoot><tr><th scope="row">Closing stock</th><td id="ledger-flow-close">0</td></tr></tfoot>
      </table>
    </details>
    <p class="works-help">Trading cash flow includes workshop openings. Berth fares and shipyard bills are kept separately.</p>
  </div>`;
  const recipe = $('works-recipe');
  for (let g=0;g<3;g++) {
    const row=document.createElement('li'); row.id='works-input-'+g;
    row.innerHTML='<canvas aria-hidden="true"></canvas><span></span><span class="works-avail"></span>';
    paintCargo(row.querySelector('canvas'),g);recipe.appendChild(row);
  }
  const inventory=$('ledger-inventory');
  worksGoods.forEach((name,g)=>{
    const row=document.createElement('tr');
    row.innerHTML='<th scope="row">'+name+'</th><td id="ledger-stock-'+g+'"></td><td id="ledger-bid-'+g+'"></td><td id="ledger-value-'+g+'"></td>';
    inventory.appendChild(row);
  });
  const flowGood=$('ledger-flow-good'), flowRows=$('ledger-flow-rows');
  worksGoods.forEach((name,g)=>{
    const option=document.createElement('option');option.value=g;option.textContent=name;flowGood.appendChild(option);
  });
  flowGood.onchange=renderLedger;
  ['Opening stock','Landed at berth','Bought at market','Made in workshop','Sold at market','Used at works or shipyard','Delivered to orders and neighbors'].forEach((label,k)=>{
    const row=document.createElement('tr');
    row.innerHTML='<th scope="row">'+label+'</th><td id="ledger-flow-'+k+'"></td>';
    flowRows.appendChild(row);
  });
  worksNames.forEach((_,i)=>{
    const button=$('works-kind-'+i);
    paintCargo(button.querySelector('canvas'), i===0?1:2);
    button.onclick=()=>{currentWorks=i;worksSceneKey='';worksNoticeUntil=0;renderWorks();};
  });
  $('works-build').onclick=()=>{
    actStart();
    if(wasm.factory_unlock(currentWorks)){
      worksMessage(worksNames[currentWorks]+' opened. Its batch ticket is now available.');
      sound([349,440,523]);
    }else worksMessage('The workshop could not open. Check its cost against your cash.');
    finishAction();
  };
  $('works-start').onclick=()=>{
    actStart();
    if(wasm.factory_start(currentWorks)){
      worksMessage('Batch started. Inputs and processing fee have been paid.');
      sound([330,440]);
    }else worksMessage('Batch could not start. Check available inputs, cash and storage.');
    finishAction();
  };
  $('works-repeat').onclick=()=>{
    const requested=$('works-repeat').getAttribute('aria-pressed')!=='true';
    actStart();
    wasm.factory_set_repeat(currentWorks,requested?1:0);
    worksMessage(wasm.factory_repeat(currentWorks)?
      'Repeat is on. Each fresh batch draws inputs and pays its own fee.':
      'Repeat is off. A batch already underway will finish.');
    finishAction();
  };
  $('works-sell-one').onclick=()=>sellWorks(false);
  $('works-sell-all').onclick=()=>sellWorks(true);
}

function sellWorks(all) {
  const button=$(all?'works-sell-all':'works-sell-one');
  const qty=Number(button.dataset.qty), amount=Number(button.dataset.amount);
  actStart();
  const paid=wasm.factory_sell(currentWorks,all?1:0,qty,amount);
  if(paid>0) {
    worksMessage('Sold '+worksAmount(qty)+(qty===1?' crate':' crates')+' of '+worksGoods[wasm.factory_good(currentWorks)].toLowerCase()+' for '+worksAmount(paid)+' coins.');
    sound([523,659]);
  } else worksMessage('The sale ticket changed. Check the current quantity and price before selling.');
  finishAction();
}

function renderLedger() {
  const entries = Array.from({length:5},(_,k)=>wasm.ledger_value(k));
  ['sales','purchases','fees','construction','orders'].forEach((key,k)=>setText($('ledger-'+key),worksAmount(entries[k])));
  const receipts=entries[0]+entries[4], paid=entries[1]+entries[2]+entries[3], flow=receipts-paid;
  setText($('ledger-receipts'),num(receipts));setText($('ledger-paid'),num(paid));
  setText($('ledger-net'),(flow<0?'−':flow>0?'+':'')+num(Math.abs(flow)));
  setText($('ledger-receipts-total'),worksAmount(receipts));setText($('ledger-paid-total'),worksAmount(paid));
  $('ledger-net').title=worksSigned(flow)+' coins';
  let total=0, unfilled=0;
  for(let g=0;g<worksGoods.length;g++){
    const stock=g<3?wasm.freight_stock(g):wasm.factory_stock(g-3);
    // The per-good sale price changes across the depth of the merchant's book.
    const sellQty=wasm.market_qty(g,0,1), sale=wasm.market_total(g,0,1);
    setText($('ledger-stock-'+g),worksAmount(stock));setText($('ledger-bid-'+g),worksAmount(sellQty));
    setText($('ledger-value-'+g),worksAmount(sale));total+=sale;unfilled+=stock-sellQty;
  }
  setText($('ledger-inventory-total'),worksAmount(total));
  setText($('ledger-liquid'),worksAmount(wasm.cash()+total)+' coins');
  setText($('ledger-depth'),unfilled ? worksAmount(unfilled)+(unfilled===1?' crate remains':' crates remain')+' without a buyer at this tide.':'The quoted values include each price step through current market depth.');
  const selected=Number($('ledger-flow-good').value);
  const flows=Array.from({length:7},(_,k)=>wasm.ledger_flow(selected,k));
  flows.forEach((count,k)=>setText($('ledger-flow-'+k),(count&&k>=4?'−':'')+worksAmount(count)));
  setText($('ledger-flow-close'),worksAmount(selected<3?wasm.freight_stock(selected):wasm.factory_stock(selected-3)));

}

function renderWorks() {
  if(!wasm||!$('exchange-book').open) return;
  if(!$('ledger-panel').hidden) renderLedger();
  if($('works-panel').hidden) return;
  const i=currentWorks,g=wasm.factory_good(i),good=worksGoods[g],open=!!wasm.factory_open(i);
  const busy=!!wasm.factory_running(i),repeat=!!wasm.factory_repeat(i);
  const cost=wasm.factory_unlock_cost(i),fee=wasm.factory_fee(i),duration=wasm.factory_duration(i);
  const elapsed=wasm.factory_elapsed(i),pct=busy?Math.min(99,Math.floor(elapsed*100/Math.max(1,duration))):0;
  const qty=wasm.factory_output(i),stock=wasm.factory_stock(i),cap=wasm.factory_capacity(i);
  const canStart=!!wasm.factory_can_start(i),gross=wasm.factory_product_value(i),inputs=wasm.factory_input_value(i),margin=wasm.factory_margin(i);
  worksNames.forEach((_,k)=>$('works-kind-'+k).setAttribute('aria-pressed',String(k===i)));
  setText($('works-title'),worksNames[i]);setText($('works-time'),worksAmount(qty)+(qty===1?' crate':' crates')+' / '+Math.ceil(duration/1000)+'s');
  const missing=[];
  let fullAskCost=0,wholeRecipeOffered=true,canSellInputs=true,shortageCost=0,shortageAvailable=true;
  for(let raw=0;raw<3;raw++){
    const needed=wasm.factory_input(i,raw),held=wasm.freight_stock(raw),row=$('works-input-'+raw);
    row.hidden=!needed;
    if(!needed) continue;
    const parts=row.querySelectorAll('span');
    setText(parts[0],worksAmount(needed)+' '+worksGoods[raw].toLowerCase());
    setText(parts[1],worksAmount(held)+' in hold');parts[1].classList.toggle('shortage',held<needed);
    const purchase=wasm.market_ask_lot(raw,needed);
    if(purchase) fullAskCost+=purchase;else wholeRecipeOffered=false;
    if(!wasm.market_bid_lot(raw,needed)) canSellInputs=false;
    if(held<needed) {
      const short=needed-held,ask=wasm.market_ask_lot(raw,short);
      missing.push(short+' '+worksGoods[raw].toLowerCase());
      if(ask) shortageCost+=ask;else shortageAvailable=false;
    }
  }
  setText($('works-status'),!open?'Awaiting opening':busy?'Batch underway':repeat?'Repeat enabled':'Idle');
  setText($('works-clock'),busy?marketSeconds(Math.max(0,duration-elapsed)):'');
  $('works-fill').style.width=pct+'%';$('works-meter').setAttribute('aria-valuenow',String(pct));
  const build=$('works-build'),start=$('works-start'),auto=$('works-repeat');
  build.hidden=open;start.hidden=auto.hidden=!open;
  setText(build,'Open workshop · '+worksAmount(cost)+' coins');build.disabled=wasm.cash()<cost;
  setText(start,busy?'Producing…':'Start batch · '+worksAmount(fee));start.disabled=!canStart;
  setText(auto,repeat?'Repeat on ✓':'Repeat off');auto.setAttribute('aria-pressed',String(repeat));
  const status=!open?(wasm.cash()<cost?'Needs '+worksAmount(cost-wasm.cash())+' more coins to open.':worksRecipes[i]):
    busy?'Inputs and fee are paid. Finished goods go to the hold.':
    stock+qty>cap?'Product storage is full. Sell some '+good.toLowerCase()+' to continue.':
    missing.length?'Needs '+missing.join(' and ')+'. '+(shortageAvailable?'Buy the shortfall for '+worksAmount(shortageCost)+' coins at the current ask, or land it at the berth.':'The exchange cannot fill the shortfall this tide. Land the cargo at its berth.'): 
    wasm.cash()<fee?'Needs '+worksAmount(fee-wasm.cash())+' more coins for the processing fee.':
    worksRecipes[i];
  setText($('works-help'),status);
  setText($('works-output-value'),worksAmount(gross));setText($('works-input-value'),'−'+worksAmount(inputs));
  setText($('works-fee'),'−'+worksAmount(fee));setText($('works-next-margin'),worksSigned(margin));
  $('works-next-margin').classList.toggle('loss',margin<0);
  const canSellOutput=wasm.market_bid_lot(g,qty)>0;
  setText($('works-purchased-margin'),!wholeRecipeOffered?'Limited supply':!canSellOutput?'No buyer':worksSigned(gross-fullAskCost-fee));
  $('works-purchased-margin').title=wholeRecipeOffered?'Full input bill at the ask: '+worksAmount(fullAskCost)+' coins.':'The merchant cannot fill the whole input order.';
  setText($('works-value-help'),!canSellOutput?'There is no buyer for this output now. Check again next tide.':
    !canSellInputs?'The merchant cannot buy the full raw recipe. Input value counts only saleable crates.':
    'The buying comparison charges the full recipe at current asks. Realized gains depend on the price when you sell.');
  setText($('works-stock-label'),good+' in hold');setText($('works-stock'),worksAmount(stock)+' / '+worksAmount(cap));
  for(let a=0;a<=1;a++){
    const n=wasm.factory_sell_qty(i,a), total=wasm.factory_sell_total(i,a),b=$(a?'works-sell-all':'works-sell-one');
    b.dataset.qty=n;b.dataset.amount=total;b.disabled=!n;
    setText(b,(a?'Sell all':'Sell one')+(n?' · '+worksAmount(total):''));
    b.title=n?'Sell '+worksAmount(n)+' for '+worksAmount(total)+' coins':'No finished stock in the hold';
  }
  const phase=busy?(reduced.matches?500:1+Math.floor(pct/5)*50):0;
  const key=i+':'+phase;
  if(key!==worksSceneKey) {
    const canvas=$('works-art');paintWorks(canvas,i,phase);
    canvas.setAttribute('aria-label',worksNames[i]+(busy?', batch in production.':', idle.'));
    worksSceneKey=key;
  }
  setText($('works-receipt'),time()<worksNoticeUntil?worksNotice:'Workshop buildings and accounts reset at the next charter.');
}
