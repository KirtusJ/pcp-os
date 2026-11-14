// The slipway reads the same commission, construction clock and naval register
// as the PCP OS guest. The launch slide is a short, purely visual celebration.
const hullPlans = [
  { name: 'Pilgrim', role: 'Pilot cutter', purpose: 'She carries the local pilot out to incoming ships. At the harbor mouth, look for her ochre sail.', inscription: '“She took the water so quietly we heard the painter’s brush fall off the cradle.” — Bo' },
  { name: 'Juniper', role: 'Packet boat', purpose: 'Juniper keeps the island mail moving on a regular crossing. Her red hull is easy to pick out on the water.', inscription: '“First bag aboard had three birthday cards for the same child. We made the morning tide.” — Nell' },
  { name: 'Northstar', role: 'Lightship', purpose: 'Anchored off the shoal, Northstar shows the fleet a way home after dark. Watch for her masthead lamp.', inscription: '“On the first night we lit her lamp, every boat coming in rang its bell.” — Inez' }
];
const yardCanvas = $('yard-view');
const yardContext = yardCanvas.getContext('2d', {alpha:false});
const yardFrame = yardContext.createImageData(360,112);
let yardSelected = 0, launchedBefore = null, yardTransition = null, yardSceneKey = '';
let lastYardRequest = '', lastBill = '', yardReceipt = '';
const hullDone = i => !!wasm.yard_count(i);
const yardRemaining = i => Math.max(0, Math.ceil((wasm.yard_duration(i) - wasm.yard_elapsed()) / 1000));

function initYard() {
  paintHull($('yard-seal'), 3);
  hullPlans.forEach((plan, i) => {
    const b = $('yard-plan-' + i);
    paintHull(b.querySelector('canvas'), i);
    b.onclick = () => { yardSelected = i; yardSceneKey = ''; yardReceipt = ''; renderYard(); };
  });
  const dialog = $('yard-book');
  $('shipyard').onclick = () => {
    if (!wasm) return;
    actStart();
    const next = wasm.yard_next();
    yardSelected = next < 3 ? next : 2;
    yardSceneKey = ''; yardReceipt = '';
    dialog.showModal();
    renderYard();
    $('close-yard').focus();
  };
  $('close-yard').onclick = () => dialog.close();
  dialog.addEventListener('click', e => {
    if (e.target !== dialog) return;
    const r = dialog.getBoundingClientRect();
    if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) dialog.close();
  });
  $('yard-build').onclick = () => {
    actStart();
    const i = yardSelected;
    if (wasm.yard_begin(i)) {
      lastBill = '';
      yardReceipt = 'Keel laid. The crew has signed for the materials.';
      sound([294, 370, 440]);
    } else {
      yardReceipt = 'The slipway register has changed. Check the requisition before laying the keel.';
    }
    finishAction();
  };
  $('yard-certificate').onclick = saveYardCertificate;
  $('yard-provisions').onclick = () => {
    const good = goods.findIndex((_, g) => wasm.freight_stock(g) < wasm.yard_cargo(yardSelected, g));
    if (good >= 0) tradedGood = good;
    dialog.close();
    $('exchange').click();
  };
}

function saveYardCertificate() {
  const i = yardSelected;
  if (!hullDone(i)) return;
  // The document records the ship in service, including when saved during the launch slide.
  const scene = document.createElement('canvas');
  scene.width = 360; scene.height = 112;
  const context = scene.getContext('2d'), image = context.createImageData(360,112);
  const ptr = wasm.render_yard(120,i,0);
  blitRgb(new Uint32Array(wasm.memory.buffer,ptr,360*112),image,context);
  const card = document.createElement('canvas'), plan = hullPlans[i];
  drawShipCertificate(card, scene, i, plan.name, plan.role, wasm.yard_award(i));
  card.toBlob(blob => {
    if (!blob) { yardReceipt = 'The certificate could not be drawn. Try saving again.'; renderYard(); return; }
    const url = URL.createObjectURL(blob), link = document.createElement('a');
    link.href = url; link.download = 'Bellweather_' + plan.name + '_Certificate.png';
    document.body.appendChild(link); link.click(); link.remove(); setTimeout(() => URL.revokeObjectURL(url), 2000);
  }, 'image/png');
}
function drawSlipway(stage, launchStage, phase) {
  // Reduced motion keeps the ship still. The drawings still show real building progress.
  const tick = reduced.matches ? 0 : Math.floor(artTick * .03);
  const key = [yardSelected, stage, launchStage, phase, tick].join('/');
  if (yardSceneKey === key) return;
  yardSceneKey = key;
  const ptr = wasm.render_yard(tick, yardSelected, launchStage);
  const source = new Uint32Array(wasm.memory.buffer, ptr, 360 * 112);
  blitRgb(source, yardFrame, yardContext);
}
function updateRequisition(i, fitting, complete, locked) {
  const bill = [{name:'Coins',need:wasm.yard_cost(i),held:wasm.cash()}];
  for (let g = 0; g < goods.length; g++) {
    const need = wasm.yard_cargo(i,g);
    if (need) bill.push({name:goods[g],need,held:wasm.freight_stock(g)});
  }
  const signature = [i,fitting,complete].concat(bill.map(v => fitting || complete ? '' : v.held)).join('/');
  if (signature !== lastBill) {
    lastBill = signature;
    const holder = $('yard-bill');
    holder.textContent = '';
    bill.forEach(v => {
      const line = document.createElement('div');
      const key = document.createElement('span'), value = document.createElement('strong');
      line.className = 'yard-line';
      if (v.held < v.need && !fitting && !complete) line.classList.add('short');
      key.textContent = v.name;
      value.textContent = fitting || complete ? num(v.need) + ' paid' : num(v.held) + ' / ' + num(v.need);
      value.title = fitting || complete ? 'Already delivered to the yard' : v.held + ' available, ' + v.need + ' needed';
      line.append(key, value); holder.appendChild(line);
    });
  }
  const missing = bill.filter(v => v.held < v.need);
  const b = $('yard-build');
  b.disabled = !wasm.yard_can_begin(i);
  setText(b, complete ? 'Launched ✓' : fitting ? 'Building on the ways' : 'Lay the keel');
  let help = 'Cargo and coins are paid when you lay the keel.';
  if (complete) help = 'This vessel stays on the register through every charter.';
  else if (fitting) help = 'Paid in full. The yard keeps working while you’re away.';
  else if (locked) help = 'Launch ' + hullPlans[i-1].name + ' before laying this keel.';
  else if (missing.length) help = 'Still needed: ' + missing.map(v => num(v.need-v.held) + ' ' + v.name.toLowerCase()).join(', ') + '.';
  setText($('yard-help'), help);
  $('yard-provisions').hidden = fitting || complete;
}

function renderYard() {
  const next = wasm.yard_next(), active = wasm.yard_active();
  const count = hullPlans.reduce((n, _, i) => n + Number(hullDone(i)), 0);
  if (launchedBefore !== null && count > launchedBefore) {
    const i = count - 1;
    yardReceipt = hullPlans[i].name + ' is in service. Business payouts gained ' + wasm.yard_award(i) + ' percentage points.';
    note(yardReceipt);
    if ($('yard-book').open && yardSelected === i) {
      if (!reduced.matches) yardTransition = {ship:i,start:time()};
      sound([294, 370, 440, 587]);
    }
    draw(time(), true);
  }
  if (launchedBefore !== null && count < launchedBefore) { yardTransition = null; yardReceipt = ''; }
  launchedBefore = count;
  const summary = active < 3 ? hullPlans[active].name + ' on the ways · ' + yardRemaining(active) + 's' :
    next < 3 ? (wasm.yard_can_begin(next) ? 'Ready to lay ' + hullPlans[next].name + '’s keel.' :
    count ? count + ' in service · ' + hullPlans[next].name + ' is next.' : 'Build Pilgrim with four salt crates and 24 coins.') :
    'All three in service · +' + wasm.yard_bonus() + '% business payouts';
  setText($('yard-summary'), summary);
  if (!$('yard-book').open) return;
  const i = yardSelected, plan = hullPlans[i];
  const complete = hullDone(i), fitting = i === active, locked = !complete && i > next;
  const elapsed = fitting ? wasm.yard_elapsed() : 0, duration = wasm.yard_duration(i);
  const progress = fitting ? Math.floor(1000 * elapsed / duration) : complete ? 1000 : 0;
  let launchStage = 0;
  if (yardTransition) {
    const age = time() - yardTransition.start;
    if (age >= 2400) yardTransition = null;
    else if (yardTransition.ship === i && complete && !reduced.matches) launchStage = 1001 + Math.floor(age * 999 / 2400);
  }
  setText($('yard-total'), count + ' / 3'); setText($('yard-bonus'), '+' + wasm.yard_bonus() + '%');
  hullPlans.forEach((h, j) => {
    const b = $('yard-plan-' + j);
    b.setAttribute('aria-pressed', String(i === j));
    b.setAttribute('aria-label', h.name + ', ' + h.role + ', ' +
      (hullDone(j) ? 'launched' : active === j ? 'under construction' : j === next ? 'next for construction' : 'future plan'));
  });
  setText($('hull-name'), plan.name);
  const state = launchStage ? 'Launching' : complete ? 'In service' : fitting ? 'Under construction' :
    locked ? 'Future plan' : wasm.yard_can_begin(i) ? 'Ready to build' : 'Awaiting stores';
  setText($('yard-status'), state); setText($('hull-purpose'), plan.purpose);
  setText($('hull-seconds'), duration / 1000 + 's'); setText($('hull-bonus'), '+' + wasm.yard_award(i) + '%');
  $('yard-work').hidden = !fitting; $('yard-progress').value = progress;
  const phase = progress < 240 ? 'Laying the keel' : progress < 570 ? 'Planking the hull' : progress < 850 ? 'Stepping the mast' : 'Rigging for launch';
  setText($('yard-phase'), phase); setText($('yard-eta'), fitting ? yardRemaining(i) + 's' : '');
  setText($('yard-inscription'), complete ? plan.inscription : ''); $('yard-inscription').hidden = !complete;
  $('yard-certificate').hidden = !complete;
  updateRequisition(i, fitting, complete, locked);
  setText($('yard-receipt'), yardReceipt || 'The register and paid work carry over to your next charter.');
  const description = plan.name + ', ' + plan.role.toLowerCase() + '. ' + (fitting ? phase : state) + '.';
  if (description !== lastYardRequest) { yardCanvas.setAttribute('aria-label', description); lastYardRequest = description; }
  drawSlipway(progress, launchStage, state);
}
