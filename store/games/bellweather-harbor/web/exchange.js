// All receipts and inventory changes are settled by the shared Rust economy.
const goods = ['Salt', 'Crab', 'Timber'];
const cargoSources = ['rowboat trip', 'crab-pot haul', 'ferry crossing'];
const consignees = [
  ['Light Station Stores', 'Lantern Fishery', 'Lighthouse Works'],
  ['Beacon Chandlery', 'Gull Rock Kitchen', 'North Pier Shipwrights'],
  ['Ferry Provisions', 'The Last Ferry Inn', 'Pine Key Boatyard'],
  ['Outer Sound Stores', 'Moth Bank Cannery', 'Nell’s Repair Yard']
];
let tradedGood = 0, requestedLot = 1, exchangeTab = 'market';
const exchangeTabs = [
  ['market','market-panel'], ['works','works-panel'], ['broker','broker-panel'], ['accounts','ledger-panel']
];
const tradeTickets = [
  ['sell-one',false,false], ['sell-all',false,true], ['buy-one',true,false], ['buy-max',true,true]
];
function selectExchangeTab(tab) {
  exchangeTab=tab;
  exchangeTabs.forEach(([key,panel])=>{
    $(panel).hidden=key!==tab;
    $('tab-'+key).setAttribute('aria-pressed', String(key===tab));
  });
  $('market-scene').hidden=tab!=='market';
  $('exchange-receipt').hidden=tab!=='market';
  if(wasm)renderFreight();
}
let sceneCode = '', historyCode = '', lastContract = null;
let exchangeFeedback = '', receiptUntil = 0;

function marketSeconds(ms) {
  const seconds = Math.ceil(ms / 1000);
  return Math.floor(seconds / 60) + ':' + String(seconds % 60).padStart(2, '0');
}
function marketMessage(message) {
  exchangeFeedback = message;
  receiptUntil = time() + 6500;
  setText($('exchange-receipt'), message);
}
function initExchange() {
  paintCargo($('exchange-seal'), 3);
  exchangeTabs.forEach(([tab])=>{ $('tab-'+tab).onclick=()=>selectExchangeTab(tab); });
  const lotInput=$('ticket-qty');
  function setSize(n) {
    requestedLot=Math.min(96,Math.max(1,Math.floor(n)||1));
    lotInput.value=requestedLot;
    if(wasm)renderFreight();
  }
  $('lot-less').onclick=()=>setSize(requestedLot-1);
  $('lot-more').onclick=()=>setSize(requestedLot+1);
  lotInput.addEventListener('input',()=>{
    if(lotInput.value!=='')setSize(lotInput.valueAsNumber);
  });
  lotInput.addEventListener('blur',()=>setSize(lotInput.valueAsNumber));
  const depthBody=$('market-depth').querySelector('tbody');
  [1,5,10,24].forEach(q=>{
    const tr=document.createElement('tr');
    tr.dataset.depth=q; tr.innerHTML='<td>'+q+'</td><td></td><td></td>'; depthBody.appendChild(tr);
  });
  tradeTickets.forEach(([id,buy,all])=>{
    $(id).innerHTML='<b></b><small></small>';
    $(id).onclick=()=>tradeFreight(id,buy,all);
  });
  goods.forEach((good, i) => {
    const button = $('good-' + i);
    const canvas = button.querySelector('canvas');
    canvas.setAttribute('aria-hidden', 'true');
    paintCargo(canvas, i);
    button.onclick = () => {
      tradedGood = i;
      renderFreight();
    };
  });
  const dialog = $('exchange-book');
  $('exchange').onclick = () => {
    if (!wasm) return;
    actStart();
    sceneCode = ''; historyCode = '';
    dialog.showModal();
    selectExchangeTab('market');
    $('close-exchange').focus();
  };
  $('close-exchange').onclick = () => dialog.close();
  dialog.addEventListener('click', event => {
    if (event.target !== dialog) return;
    const bounds = dialog.getBoundingClientRect();
    if (event.clientX < bounds.left || event.clientX > bounds.right ||
        event.clientY < bounds.top || event.clientY > bounds.bottom) dialog.close();
  });

  $('freight-dispatch').onclick = () => {
    actStart();
    if (wasm.dispatch(tradedGood)) {
      marketMessage(names[tradedGood] + ' dispatched. One crate will land when the trip is complete.');
      sound([330, 440]);
    }
    finishAction();
  };
  $('reserve-order').onclick = () => {
    const displayedTide = Number($('reserve-order').dataset.tide);
    actStart();
    if (wasm.freight_tide() !== displayedTide) {
      marketMessage('The tide changed. Review the new order before reserving.');
    } else if (wasm.freight_send()) {
      const quote = wasm.freight_offer_pay();
      marketMessage('Manifest signed. ' + quote + ' coins due when the tide turns.');
      sound([392, 494, 587]);
    } else {
      marketMessage('The order needs ' + wasm.freight_offer_units() + ' crates in the hold.');
    }
    finishAction();
  };
  $('upgrade-hold').onclick = () => {
    actStart();
    if (wasm.freight_upgrade()) {
      marketMessage('New storage is open. The hold now takes ' + wasm.freight_capacity() + ' crates of each good.');
      sound([440, 587, 740]);
    }
    finishAction();
  };
}
function tradeFreight(id,buy,all) {
  const ticket=$(id), quantity=Number(ticket.dataset.quantity), total=Number(ticket.dataset.total);
  actStart();
  const filled=all?wasm.market_trade(tradedGood,Number(buy),1,quantity,total):
    wasm.market_trade_lot(tradedGood,Number(buy),requestedLot,quantity,total);
  if(!filled){
    marketMessage('The lot changed. Review the current quantity and total before trading.');
  }else{
    marketMessage((buy?'Bought ':'Sold ')+quantity+' '+goods[tradedGood].toLowerCase()+
      (quantity===1?' crate':' crates')+' for '+num(filled)+' coins.');
    sound(buy?[392,494]:[523,659]);
  }
  finishAction();
}
function renderTickets(i) {
  tradeTickets.forEach(([id,buy,all])=>{
    const button=$(id), qty=all?wasm.market_qty(i,+buy,1):wasm.market_lot_qty(i,+buy,requestedLot);
    const total=all?wasm.market_total(i,+buy,1):wasm.market_lot_total(i,+buy,requestedLot);
    const possible=all?qty>0:qty===requestedLot;
    const label=(buy?'Buy ':'Sell ')+(all?'max'+(qty?' ×'+qty:''):requestedLot===1?'one':'×'+requestedLot);
    button.disabled=!possible;
    button.dataset.quantity=qty; button.dataset.total=total;
    setText(button.firstElementChild,label);setText(button.lastElementChild,possible?num(total)+' coins':'Unavailable');
    button.setAttribute('aria-label',label+' '+goods[i]+(qty?', '+total+' coins':' unavailable'));
  });
  $('lot-less').disabled=requestedLot===1;$('lot-more').disabled=requestedLot===96;
  if($('book-depth').open){
    $('market-depth').querySelectorAll('[data-depth]').forEach(tr=>{
      const size=Number(tr.dataset.depth), sell=wasm.market_bid_lot(i,size), buy=wasm.market_ask_lot(i,size);
      setText(tr.cells[1],sell?num(sell):'—'); setText(tr.cells[2],buy?num(buy):'—');
    });
  }
}
function drawPrices() {
  const length=wasm.market_history_len();
  const closing=Array.from({length},(_,i)=>wasm.market_close(tradedGood,i));
  // Zero denotes a price the merchant cannot currently execute.
  const quotes=closing.concat(wasm.market_bid(tradedGood));
  const valid=quotes.filter(price=>price>0);
  setText($('chart-key'),length?length+' closes + live bid':'Opening bid');
  const signature = tradedGood + ':' + quotes.join(',');
  if (signature === historyCode) return;
  historyCode = signature;
  const canvas = $('price-chart'), c = canvas.getContext('2d');
  const low = valid.length?Math.min.apply(null,valid):0, high = valid.length?Math.max.apply(null,valid):0;
  const coords=quotes.map((v,i)=>[quotes.length===1?174:6+i*168/(quotes.length-1),
    high===low?23:38-30*(v-low)/(high-low),v>0]);
  c.clearRect(0, 0, canvas.width, canvas.height);
  c.strokeStyle = '#c5c8aa'; c.lineWidth = 1;
  [8, 23, 38].forEach(y => { c.beginPath(); c.moveTo(5, y + .5); c.lineTo(179, y + .5); c.stroke(); });
  c.beginPath();
  let joined=false;
  coords.forEach(([x,y,available])=>{
    if(!available){joined=false;return}
    if(joined)c.lineTo(x,y);else c.moveTo(x,y);
    joined=true;
  });
  c.lineWidth=3;c.lineJoin='round';c.strokeStyle='#387570';c.stroke();
  coords.forEach(([x,y,available],idx)=>{
    if(available){c.fillStyle=idx===coords.length-1?'#a44e31':'#387570';c.fillRect(x-2,y-2,4,4)}
  });
  const label=goods[tradedGood]+' closing bids followed by the live bid: '+quotes.map(v=>v||'no bid').join(', ')+'.';
  canvas.setAttribute('aria-label', label); canvas.title = label;
}
function renderFreight() {
  const tide = wasm.freight_tide();
  const pending = !!wasm.freight_pending();
  const stock = goods.map((_, i) => wasm.freight_stock(i));
  const total = stock.reduce((a, b) => a + b, 0);
  if (lastContract && tide > lastContract.tide && !pending) {
    marketMessage(lastContract.buyer + ' paid ' + num(lastContract.amount) + ' coins. The shipment has sailed.');
  }
  const offerGood = wasm.freight_offer_good();
  const buyer = consignees[Math.floor(tide / 3) % consignees.length][offerGood];
  lastContract = pending ? {tide, buyer, amount:wasm.freight_pending_pay()} : null;
  setText($('freight-summary'), pending ? 'Booked shipment · ' + num(wasm.freight_pending_pay()) + ' coins due' :
    total ? total + (total === 1 ? ' crate ashore · ' : ' crates ashore · ') + 'Spot market open' :
      'Land a crate on your next rowboat trip.');
  if (!$('exchange-book').open) return;

  const remaining=wasm.freight_tide_ms();
  setText($('market-tide'),String(tide+1).padStart(2,'0'));
  setText($('tide-time'),marketSeconds(remaining));setText($('exchange-cash'),num(wasm.cash()));
  if(exchangeTab==='broker'){renderBroker();return}
  if(exchangeTab!=='market'){renderWorks();return}
  const i=tradedGood, amount=stock[i], cap=wasm.freight_capacity();
  const current=wasm.market_bid(i),ask=wasm.market_ask(i);
  const offerQty=wasm.freight_offer_units(),offerPay=wasm.freight_offer_pay();
  const priceHere=wasm.market_bid_lot(offerGood,offerQty),upgradePrice=wasm.freight_upgrade_cost();
  for (let g = 0; g < goods.length; g++) {
    const button = $('good-' + g);
    button.setAttribute('aria-pressed', String(g === i));
    button.setAttribute('aria-label', goods[g] + ', ' + stock[g] + ' crates held');
  }
  setText($('spot-price'),current?String(current):'—');setText($('ask-price'),ask?String(ask):'—');
  const outside=[];
  if(wasm.market_rival(0))outside.push('The town cannery expects to buy 1 salt and 2 crab and deliver 1 provisions crate.');
  if(wasm.market_rival(1))outside.push('The town chandler expects to buy 1 salt and 2 timber and deliver 1 fittings crate.');
  setText($('rival-note'),outside.length?outside.join(' '):'No outside workshop batch is expected at current prices.');
  setText($('merchant-stock'),wasm.market_stock(i)+' / '+wasm.market_capacity(i));
  setText($('next-price'),wasm.market_forecast(i)||'—');
  setText($('stock-now'), amount);
  setText(document.querySelector('.stock-limit'), cap);
  $('cargo-fill').style.width = (100 * amount / cap) + '%';
  setText($('cargo-source'), amount >= cap ? 'Hold full. Sell or reserve cargo to make room for the next landing.' :
    wasm.shop_owned(i) ? 'Each ' + cargoSources[i] + ' lands one crate here.' :
    'Buy here or open ' + names[i] + ' to land your own crates.');
  const source = $('freight-dispatch');
  const owned = wasm.shop_owned(i), underway = wasm.shop_running(i);
  source.hidden = !owned;
  source.disabled = !owned || !!underway || amount >= cap;
  const arriving = Math.ceil((wasm.shop_duration(i) - wasm.shop_elapsed(i)) / 1000);
  setText(source, !underway ? depart[i] : (wasm.shop_managed(i) ? 'Crew · ' : 'Due in ') + arriving + 's');
  renderTickets(i);
  setText($('order-state'), pending ? 'Signed manifest' : 'Freight order');
  setText($('order-title'), buyer);
  setText($('order-info'), offerQty + ' crates of ' + goods[offerGood].toLowerCase() +
    (pending ? ' aboard · ' : ' for ') + num(offerPay) + ' coins');
  setText($('order-compare'), priceHere ? 'Sell the same lot now for '+num(priceHere)+' coins · '+
    (offerPay>=priceHere?'+':'−')+num(Math.abs(offerPay-priceHere))+' on order' : 'Spot buyer cannot take this full lot.');
  $('reserve-order').dataset.tide = tide;
  $('reserve-order').disabled = pending || stock[offerGood] < offerQty;
  setText($('reserve-order'), pending ? 'Shipment booked ✓' : 'Reserve ' + offerQty + ' crates');
  setText($('order-hint'), pending ? 'Payment arrives next tide, even if you close the game.' :
    stock[offerGood] < offerQty ? 'Need ' + (offerQty - stock[offerGood]) + ' more ' +
      goods[offerGood].toLowerCase() + '. Payment arrives next tide.' : 'Load the crates now. Collect payment next tide.');
  setText($('hold-cap'), cap);
  setText($('upgrade-hold'), upgradePrice ? 'Expand · ' + num(upgradePrice) : 'Fully expanded');
  $('upgrade-hold').disabled = !upgradePrice || wasm.cash() < upgradePrice;
  $('upgrade-hold').title = upgradePrice ? 'Double space for every good to ' + (cap * 2) + ' crates' : 'Maximum capacity';
  if (time() >= receiptUntil) {
    setText($('exchange-receipt'), 'Quotes move as crates trade. Outside deliveries and demand arrive every 30 seconds.');
  } else {
    setText($('exchange-receipt'), exchangeFeedback);
  }
  drawPrices();
  const scene = [tide % 12, i].concat(stock).join(',');
  if (scene !== sceneCode) {
    sceneCode = scene;
    paintExchange($('market-art'), tide % 12, stock, i);
  }
}
