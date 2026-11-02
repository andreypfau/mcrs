(() => {
  const reduced = matchMedia('(prefers-reduced-motion: reduce)').matches;
  const fig = document.querySelector('.ecs');
  const range = document.getElementById('tick-range');
  if (!fig || !range) return;

  const readK = fig.querySelectorAll('[data-read-k]');
  const readV = fig.querySelectorAll('[data-read-v]');
  const setRead = (pairs) => pairs.forEach(([k, v], i) => { readK[i].textContent = k; readV[i].textContent = v; });

  const lines = fig.querySelectorAll('.tl');
  let line = 0;
  let walker;
  const walk = (on) => {
    clearInterval(walker);
    lines.forEach((l) => l.classList.remove('on'));
    lines[0].classList.add('on');
    if (!on || reduced) return;
    line = 0;
    walker = setInterval(() => {
      lines[line].classList.remove('on');
      line = (line + 1) % lines.length;
      lines[line].classList.add('on');
    }, 420);
  };

  const systems = {
    gravity: { reads: ['Position'], writes: ['Velocity'], needs: ['Velocity'] },
    movement: { reads: ['Velocity'], writes: ['Position'], needs: ['Position', 'Velocity'] },
    burning: { reads: ['Burning'], writes: ['Health'], needs: ['Burning', 'Health'] },
    goals: { reads: ['Position'], writes: ['Goals'], needs: ['Goals'] },
  };
  const table = fig.querySelector('.ecs-table');
  const sysButtons = fig.querySelectorAll('button[data-sys]');
  const pick = (name) => {
    const sys = systems[name];
    sysButtons.forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.sys === name)));
    table.querySelectorAll('[data-col]').forEach((c) => {
      c.classList.toggle('r', !!sys && sys.reads.includes(c.dataset.col));
      c.classList.toggle('w', !!sys && sys.writes.includes(c.dataset.col));
    });
    table.querySelectorAll('tbody tr').forEach((tr) => {
      const has = tr.dataset.has.split(' ');
      tr.classList.toggle('dim', !!sys && !sys.needs.every((n) => has.includes(n)));
    });
  };
  sysButtons.forEach((b) => b.addEventListener('click', () => pick(b.dataset.sys)));

  const map = fig.querySelector('.section-map');
  const rows = [
    'fhhgggghhggggggf',
    'fggggsggggggshgf',
    'gggggggggggggggg',
    'gghgggggggggsggg',
    'ggggLgggddddddgg',
    'gldddddxrggggrLg',
    'ggggggggddddddgg',
    'gsggggggggggggsg',
    'ggggghgggggggggg',
    'fhgggggggsgggghf',
  ];
  const kinds = { g: '', h: 'g2', s: 'st', f: 'lf', d: 'dust', l: 'lever', L: 'lamp', r: 'rep', x: 'gap' };
  const frag = document.createDocumentFragment();
  rows.forEach((row, r) => [...row].forEach((ch, c) => {
    const cell = document.createElement('i');
    cell.className = ('cell ' + kinds[ch]).trim();
    cell.style.gridArea = `${r + 1} / ${c + 1}`;
    frag.append(cell);
  }));
  [['a', '5 / 2 / 7 / 8', 'Shard A · thread 1'], ['b', '5 / 9 / 8 / 16', 'Shard B · thread 2'], ['ab', '5 / 2 / 8 / 16', 'Shard A+B · one thread, in order']]
    .forEach(([cls, area, label]) => {
      const box = document.createElement('div');
      box.className = 'shard-box ' + cls;
      box.style.gridArea = area;
      box.innerHTML = `<span>${label}</span>`;
      frag.append(box);
    });
  map.append(frag);

  const lamps = map.querySelectorAll('.cell.lamp');
  const act = fig.querySelector('[data-act]');
  let splitTimer;
  const shardRead = () => {
    const state = map.dataset.state;
    if (state === 'apart') return [['Redstone shards', '2, one thread each'], ['Outcome', 'the same on 1 thread or 16']];
    if (state === 'linked') return [['Redstone shards', '1, merged at once'], ['Outcome', 'as if they were always one']];
    return [['Redstone shards', `1, splits in ${map.dataset.left} tick${map.dataset.left === '1' ? '' : 's'}`], ['Outcome', 'as if they were always one']];
  };
  const setShards = (state, left = '') => {
    map.dataset.state = state;
    map.dataset.left = String(left);
    act.textContent = state === 'apart' ? 'Place dust in the gap' : state === 'linked' ? 'Break the link' : 'Splitting…';
    act.disabled = state === 'splitting';
    if (fig.dataset.stage === '4') { setRead(shardRead()); placeBlocks(stages[4]); }
  };
  const toggleShards = () => {
    if (map.dataset.state === 'apart') { setShards('linked'); return; }
    if (map.dataset.state !== 'linked') return;
    let ticks = 3;
    setShards('splitting', ticks);
    clearInterval(splitTimer);
    splitTimer = setInterval(() => {
      ticks -= 1;
      if (ticks > 0) { setShards('splitting', ticks); return; }
      clearInterval(splitTimer);
      setShards('apart');
    }, 550);
  };
  act.addEventListener('click', toggleShards);
  map.querySelector('.cell.gap').addEventListener('click', toggleShards);
  lamps[0].classList.add('on');
  lamps[1].classList.add('on');
  if (!reduced) {
    let clock = false;
    setInterval(() => {
      clock = !clock;
      lamps[1].classList.toggle('on', clock);
      lamps[0].classList.toggle('on', map.dataset.state === 'apart' || clock);
    }, 600);
  }

  const nodes = fig.querySelector('.nodes-map');
  const busy = new Set([22, 23, 38, 39]);
  nodes.innerHTML = Array.from({ length: 64 }, (_, i) => `<i class="chunk${i % 16 >= 8 ? ' b' : ''}${busy.has(i) ? ' busy' : ''}" data-i="${i}"></i>`).join('');
  let handed = false;
  let handTimer;
  const handover = (on) => {
    clearInterval(handTimer);
    if (!on || reduced) return;
    handTimer = setInterval(() => {
      handed = !handed;
      busy.forEach((i) => nodes.children[i].classList.toggle('b', handed));
    }, 2200);
  };

  const track = fig.querySelector('.track');
  for (let i = 0; i < 6; i++) {
    const band = document.createElement('span');
    band.className = 'band';
    band.style.top = `${i * 30}px`;
    track.prepend(band);
  }
  const blocks = fig.querySelectorAll('[data-b]');
  blocks.forEach((b, i) => b.style.setProperty('--d', i));
  const laneLabels = fig.querySelectorAll('[data-lane]');
  const end = fig.querySelector('.tick-end');
  const cores = ['Core 1', 'Core 2', 'Core 3', 'Core 4', 'Core 5', 'Core 6'];

  const stages = {
    1: { name: 'One loop, nested',
      desc: 'The world ticks each chunk, each chunk ticks each entity, and a zombie on a pressure plate runs redstone from inside its own tick. Networking already has threads; this loop does not.',
      read: [['Simulation threads', '1'], ['Who can touch what', 'anything, from anywhere']] },
    2: { name: 'Data in tables',
      desc: 'An entity is a row. A zombie has a position, velocity, health and goals; falling sand has a block and no health; an arrow has only a position and a velocity.',
      read: [['An entity', 'a row of data'], ['Behaviour', 'not in the row: in systems']] },
    3: { name: 'Behaviour in systems',
      desc: 'A system is one rule over every row that has its columns. Two run at once unless one writes what the other touches, and one system splits its rows across cores. That is level one.',
      read: [['Systems at once', 'all that touch different data'], ['Still in order', 'everything that writes blocks']],
      lanes: cores, busy: 4, end: 72,
      at: { grav: [0, 0], burn: [1, 0], goals: [2, 0], move1: [0, 18], move2: [1, 18], rsA: [3, 0], rsB: [3, 18], fl: [3, 36], bt: [3, 54], conn: null } },
    4: { name: 'Split by what can reach what',
      desc: 'Redstone, fluids and block ticks all write the blocks. Shards split that work by what can affect what within a tick, so two machines in one section are two shards. That is level two.',
      read: null,
      lanes: cores, busy: 6, end: 36,
      at: { grav: [0, 0], burn: [1, 0], goals: [2, 0], move1: [0, 18], move2: [1, 18], rsA: [3, 0], rsB: [4, 0], fl: [5, 0], bt: [2, 18], conn: null },
      linked: { rsB: [3, 18] } },
    5: { name: 'One dimension, many machines',
      desc: 'A shard is data, not a thread. Players connect to an edge; nodes own chunks and their shards and hand them over as players move. The target is ten thousand players in one dimension.',
      read: [['Players connect to', 'the edge'], ['Chunks and shards', 'move between nodes']],
      lanes: ['Edge', 'Node A', 'Node A', 'Node B', 'Node B', 'Node B'], busy: 6, end: 36,
      at: { conn: [0, 0], grav: [1, 0], move1: [1, 18], burn: [2, 0], move2: [2, 18], goals: [3, 0], bt: [3, 18], rsA: [4, 0], fl: [4, 18], rsB: [5, 0] } },
  };

  const placeBlocks = (s) => {
    const at = s.linked && map.dataset.state !== 'apart' ? { ...s.at, ...s.linked } : s.at;
    blocks.forEach((b) => {
      const pos = at[b.dataset.b];
      b.classList.toggle('off', !pos);
      if (pos) { b.style.setProperty('--y', pos[0]); b.style.setProperty('--x', pos[1]); }
    });
    laneLabels.forEach((l, i) => {
      const busy = s.linked && map.dataset.state !== 'apart' ? i !== 4 : i < s.busy;
      l.textContent = s.lanes[i];
      l.classList.toggle('idle', !busy);
    });
    end.style.setProperty('--end', s.end);
  };

  const panes = fig.querySelectorAll('[data-show]');
  const text = document.querySelector('.stage-text');
  const stops = document.querySelectorAll('.stops button');
  let swapTimer;
  const show = (n) => {
    const s = stages[n];
    fig.dataset.stage = String(n);
    range.value = n;
    range.setAttribute('aria-valuetext', s.name);
    range.parentElement.style.setProperty('--f', String((n - 1) / 4));
    stops.forEach((b) => b.setAttribute('aria-pressed', String(b.dataset.go === String(n))));
    panes.forEach((p) => { p.hidden = !p.dataset.show.split(' ').includes(String(n)); });
    if (n === 2) pick(null);
    if (n === 3) pick([...sysButtons].find((b) => b.getAttribute('aria-pressed') === 'true')?.dataset.sys || 'gravity');
    setRead(s.read || shardRead());
    walk(n === 1);
    handover(n === 5);
    if (s.at) placeBlocks(s);
    text.classList.add('swap');
    clearTimeout(swapTimer);
    swapTimer = setTimeout(() => {
      document.querySelector('[data-stage-name]').textContent = s.name;
      document.querySelector('[data-stage-desc]').textContent = s.desc;
      text.classList.remove('swap');
    }, 200);
  };
  range.addEventListener('input', () => show(Number(range.value)));
  stops.forEach((b) => b.addEventListener('click', () => show(Number(b.dataset.go))));
  show(3);
  show(1);
})();
