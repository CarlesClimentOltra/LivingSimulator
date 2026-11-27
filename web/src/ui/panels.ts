// DOM panels: person card, building card, city stats and the newspaper. Read-only.

export interface AgentCard {
  id: number;
  region: number;
  name: string;
  age: number;
  female: boolean;
  visitor: boolean;
  home: string;
  occupation: string;
  workplace: string;
  schedule: string;
  doing: string;
  place: string;
  needs: [string, number][];
  money_cents: number;
  family: { id: number; name: string; relation: string; age: number }[];
  journal: { tick: number; when: string; text: string }[];
  housing: string;
  rent_cents: number;
  rent_arrears: number;
  businesses: string[];
  education: string;
  relationship: string;
  health: string;
  notes: string[];
}

export interface DemoSample {
  tick: number;
  population: number;
  births: number;
  deaths: number;
  marriages: number;
  immigrants: number;
  emigrants: number;
  ill: number;
  commuters_out: number;
  commuters_in: number;
}

export interface DemographyStats {
  pyramid: [number, number][];
  average_age: number;
  births_30d: number;
  deaths_30d: number;
  marriages_30d: number;
  arrivals_30d: number;
  departures_30d: number;
  ill: number;
  pregnant: number;
  couples: number;
  commuters_out: number;
  commuters_in: number;
  university_graduates: number;
  history: DemoSample[];
}

export interface FirmInfo {
  kind: string;
  open: boolean;
  owner: string;
  cash_cents: number;
  price: string;
  stock: number;
  staff: number;
  positions: number;
  revenue_yesterday_cents: number;
  costs_yesterday_cents: number;
  customers_yesterday: number;
}

export interface DaySample {
  tick: number;
  food_price: number;
  unemployment_permille: number;
  treasury: number;
  avg_savings: number;
  output: number;
  avg_rent: number;
  firms_open: number;
  firms_closed: number;
  income_tax_permille: number;
}

export interface EconomyStats {
  treasury_cents: number;
  income_tax_permille: number;
  sales_tax_permille: number;
  unemployment_permille: number;
  food_price_cents: number;
  avg_rent_cents: number;
  firms_open: number;
  firms_closed: number;
  owners: number;
  renters: number;
  output_cents: number;
  exports_cents: number;
  imports_cents: number;
  rent_paid_cents: number;
  hires: number;
  layoffs: number;
  history: DaySample[];
}

export interface BuildingCard {
  id: number;
  name: string;
  kind: string;
  floors: number;
  dwellings: number;
  present: number;
  occupants: [number, string][];
  firm: FirmInfo | null;
}

export interface Stats {
  name: string;
  population: number;
  households: number;
  buildings: number;
  employed: number;
  unemployed: number;
  students: number;
  children: number;
  retired: number;
  visitors: number;
  away: number;
  travelling: number;
  by_activity: [string, number][];
  avg_needs: [string, number][];
  avg_money_cents: number;
  trips_yesterday: number;
  meals_out_yesterday: number;
  groceries_yesterday: number;
  shelves_empty_yesterday: number;
  wages_yesterday_cents: number;
  economy: EconomyStats;
  demography: DemographyStats;
}

export interface NewsLine {
  tick: number;
  when: string;
  headline: string;
  body: string;
}

type Child = Node | string | null | undefined | false;

export function h(tag: string, attrs: Record<string, string> = {}, ...children: Child[]): HTMLElement {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
  for (const c of children) {
    if (c === null || c === undefined || c === false) continue;
    el.append(typeof c === 'string' ? document.createTextNode(c) : c);
  }
  return el;
}

export function money(cents: number, decimals = 2): string {
  const v = Math.abs(cents / 100).toLocaleString('en-US', { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
  return `${cents < 0 ? '-' : ''}$${v}`;
}

const FIRM_KINDS: Record<string, string> = {
  Public: 'Public service',
  Shop: 'Shop',
  FoodService: 'Restaurant / café',
  Leisure: 'Leisure',
  Farm: 'Farm',
  Factory: 'Factory',
  Warehouse: 'Warehouse',
  Office: 'Office',
  RealEstate: 'Real estate agency',
  Construction: 'Construction company',
};

/** Small inline SVG line chart. */
function sparkline(values: number[], format: (v: number) => string, color = '#64b5f6'): HTMLElement {
  const wrap = h('div', { class: 'spark' });
  if (values.length < 2) {
    wrap.append(h('span', { class: 'muted' }, 'Not enough data yet.'));
    return wrap;
  }
  const w = 280;
  const hgt = 46;
  const pad = 3;
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const pts = values
    .map((v, i) => `${(pad + (i / (values.length - 1)) * (w - 2 * pad)).toFixed(1)},${(hgt - pad - ((v - min) / span) * (hgt - 2 * pad)).toFixed(1)}`)
    .join(' ');
  const ns = 'http://www.w3.org/2000/svg';
  const svg = document.createElementNS(ns, 'svg');
  svg.setAttribute('viewBox', `0 0 ${w} ${hgt}`);
  svg.setAttribute('class', 'spark-svg');
  const line = document.createElementNS(ns, 'polyline');
  line.setAttribute('points', pts);
  line.setAttribute('fill', 'none');
  line.setAttribute('stroke', color);
  line.setAttribute('stroke-width', '2');
  line.setAttribute('stroke-linejoin', 'round');
  svg.append(line);
  wrap.append(
    svg,
    h('div', { class: 'spark-range' }, h('span', {}, `min ${format(min)}`), h('span', {}, `now ${format(values[values.length - 1])}`), h('span', {}, `max ${format(max)}`)),
  );
  return wrap;
}

function bar(label: string, value: number): HTMLElement {
  const pct = Math.round(value / 100);
  const cls = pct < 25 ? 'low' : pct < 55 ? 'mid' : 'high';
  return h('div', { class: 'need' }, h('span', { class: 'need-label' }, label), h('div', { class: 'need-bar' }, h('div', { class: `need-fill ${cls}`, style: `width:${pct}%` })), h('span', { class: 'need-val' }, `${pct}`));
}

export function renderAgentCard(el: HTMLElement, c: AgentCard, onRelative: (id: number) => void, following: boolean, onFollow: () => void) {
  const fam = c.family.length
    ? h(
        'ul',
        { class: 'family' },
        ...c.family.map((r) => {
          const a = h('a', { href: '#' }, r.name);
          a.addEventListener('click', (e) => {
            e.preventDefault();
            onRelative(r.id);
          });
          return h('li', {}, a, ` — ${r.relation}, ${r.age}`);
        }),
      )
    : h('p', { class: 'muted' }, c.visitor ? 'Family lives in another town.' : 'Lives alone.');
  const follow = h('button', { class: following ? 'btn active' : 'btn' }, following ? 'Following' : 'Follow');
  follow.addEventListener('click', onFollow);
  el.replaceChildren(
    h('div', { class: 'card-head' }, h('div', { class: `avatar ${c.female ? 'f' : 'm'}` }, c.name.slice(0, 1)), h('div', {}, h('h2', {}, c.name), h('div', { class: 'muted' }, `${c.age} years · ${c.occupation}`)), follow),
    h('p', { class: 'doing' }, h('b', {}, 'Now: '), `${c.doing}`, c.place && c.place !== 'on the road' ? h('span', { class: 'muted' }, ` · ${c.place}`) : null),
    h(
      'dl',
      { class: 'facts' },
      h('dt', {}, 'Home'), h('dd', {}, c.home),
      h('dt', {}, 'Work / school'), h('dd', {}, c.workplace),
      h('dt', {}, 'Agenda'), h('dd', {}, c.schedule),
      h('dt', {}, 'Money'), h('dd', {}, money(c.money_cents)),
      h('dt', {}, 'Family'), h('dd', {}, c.relationship),
      h('dt', {}, 'Education'), h('dd', {}, c.education),
      h('dt', {}, 'Health'), h('dd', { class: c.health === 'Healthy' ? '' : 'neg' }, c.health),
      h('dt', {}, 'Housing'), h('dd', {}, c.housing + (c.rent_arrears > 0 ? ` (${c.rent_arrears} month${c.rent_arrears > 1 ? 's' : ''} behind)` : '')),
      c.businesses.length ? h('dt', {}, 'Owns') : null,
      c.businesses.length ? h('dd', {}, c.businesses.join(', ')) : null,
    ),
    h('p', { class: 'notes' }, ...c.notes.map((n) => h('span', { class: 'chip' }, n))),
    h('h3', {}, 'Needs'),
    h('div', { class: 'needs' }, ...c.needs.map(([n, v]) => bar(n, v))),
    h('h3', {}, 'Family'),
    fam,
    h('h3', {}, 'Diary'),
    h('ol', { class: 'journal' }, ...c.journal.map((j) => h('li', {}, h('span', { class: 'when' }, j.when), ' ', j.text))),
  );
}

export function renderBuildingCard(el: HTMLElement, b: BuildingCard, onPerson: (id: number) => void) {
  const f = b.firm;
  const stocked = f !== null && (f.stock > 0 || ['Shop', 'Warehouse', 'Farm', 'FoodService'].includes(f.kind));
  const firm = f
    ? [
        h('h3', {}, FIRM_KINDS[f.kind] ?? f.kind),
        f.open ? null : h('p', { class: 'warn' }, 'Closed: the business went bankrupt and is waiting for a new owner.'),
        h(
          'dl',
          { class: 'facts' },
          h('dt', {}, 'Owner'),
          h('dd', {}, f.owner),
          f.kind !== 'Public' ? h('dt', {}, 'Cash') : null,
          f.kind !== 'Public' ? h('dd', { class: f.cash_cents < 0 ? 'neg' : '' }, money(f.cash_cents)) : null,
          f.price !== '—' ? h('dt', {}, 'Price') : null,
          f.price !== '—' ? h('dd', {}, f.price) : null,
          stocked ? h('dt', {}, 'Stock') : null,
          stocked ? h('dd', {}, `${f.stock.toLocaleString('en-US')} meals`) : null,
          h('dt', {}, 'Staff'),
          h('dd', {}, `${f.staff} of ${f.positions} positions`),
          f.kind !== 'Public' ? h('dt', {}, 'Yesterday') : null,
          f.kind !== 'Public'
            ? h(
                'dd',
                { class: f.revenue_yesterday_cents - f.costs_yesterday_cents < 0 ? 'neg' : 'pos' },
                `${money(f.revenue_yesterday_cents, 0)} in, ${money(f.costs_yesterday_cents, 0)} out` + (f.customers_yesterday ? ` · ${f.customers_yesterday} customers` : ''),
              )
            : null,
        ),
      ]
    : [];
  const meta = `${b.kind} · ${b.floors} floor${b.floors > 1 ? 's' : ''}${b.dwellings ? ` · ${b.dwellings} home${b.dwellings > 1 ? 's' : ''}` : ''} · ${b.present} inside`;
  el.replaceChildren(
    ...([
      h('div', { class: 'card-head' }, h('div', { class: 'avatar b' }, '⌂'), h('div', {}, h('h2', {}, b.name), h('div', { class: 'muted' }, meta))),
      ...firm,
      h('h3', {}, 'Inside now'),
      b.occupants.length
        ? h(
            'ul',
            { class: 'family' },
            ...b.occupants.map(([id, name]) => {
              const a = h('a', { href: '#' }, name);
              a.addEventListener('click', (e) => {
                e.preventDefault();
                onPerson(id);
              });
              return h('li', {}, a);
            }),
          )
        : h('p', { class: 'muted' }, 'Nobody is here right now.'),
    ].filter(Boolean) as HTMLElement[]),
  );
}

function stat(label: string, value: string | number): HTMLElement {
  return h('div', { class: 'stat' }, h('div', { class: 'stat-v' }, typeof value === 'number' ? value.toLocaleString('en-US') : value), h('div', { class: 'stat-l' }, label));
}

export function renderStats(el: HTMLElement, s: Stats) {
  const top = s.by_activity.slice(0, 8);
  const max = Math.max(1, ...top.map(([, n]) => n));
  el.replaceChildren(
    h('h2', {}, s.name),
    h(
      'div',
      { class: 'stats-grid' },
      stat('inhabitants', s.population),
      stat('households', s.households),
      stat('employed', s.employed),
      stat('job seekers', s.unemployed),
      stat('children', s.children),
      stat('retired', s.retired),
      stat('on the move', s.travelling),
      stat('visitors', s.visitors),
    ),
    h('h3', {}, 'Right now'),
    h('div', { class: 'acts' }, ...top.map(([a, n]) => h('div', { class: 'act' }, h('span', {}, a), h('div', { class: 'act-bar' }, h('div', { style: `width:${(n / max) * 100}%` })), h('span', { class: 'act-n' }, n.toLocaleString('en-US'))))),
    h('h3', {}, 'Average needs'),
    h('div', { class: 'needs' }, ...s.avg_needs.map(([n, v]) => bar(n, v))),
    h('h3', {}, 'Yesterday'),
    h(
      'div',
      { class: 'stats-grid' },
      stat('trips', s.trips_yesterday),
      stat('meals out', s.meals_out_yesterday),
      stat('grocery runs', s.groceries_yesterday),
      stat('empty shelves', s.shelves_empty_yesterday),
      stat('wages paid', money(s.wages_yesterday_cents, 0)),
      stat('avg savings', money(s.avg_money_cents, 0)),
    ),
    ...economySection(s.economy),
    ...demographySection(s.demography),
  );
}

export function renderNews(el: HTMLElement, cityName: string, news: NewsLine[]) {
  el.replaceChildren(
    h('div', { class: 'paper-title' }, `The ${cityName} Chronicle`),
    ...news.slice(0, 20).map((n) => h('article', { class: 'news' }, h('div', { class: 'when' }, n.when), h('h4', {}, n.headline), h('p', {}, n.body))),
  );
}

function economySection(e: EconomyStats): HTMLElement[] {
  const hist = e.history;
  const pct = (p: number) => `${(p / 10).toFixed(1)}%`;
  return [
    h('h3', {}, 'Economy'),
    h(
      'div',
      { class: 'stats-grid' },
      stat('unemployment', pct(e.unemployment_permille)),
      stat('meal at the shop', money(e.food_price_cents)),
      stat('average rent', money(e.avg_rent_cents, 0)),
      stat('town treasury', money(e.treasury_cents, 0)),
      stat('income tax', pct(e.income_tax_permille)),
      stat('sales tax', pct(e.sales_tax_permille)),
      stat('businesses open', e.firms_open),
      stat('closed (bankrupt)', e.firms_closed),
      stat('homeowners', e.owners),
      stat('renting', e.renters),
      stat('exports yesterday', money(e.exports_cents, 0)),
      stat('imports yesterday', money(e.imports_cents, 0)),
      stat('hired yesterday', e.hires),
      stat('laid off yesterday', e.layoffs),
    ),
    h('h3', {}, 'Unemployment (60 days)'),
    sparkline(hist.map((d) => d.unemployment_permille), pct, '#e57373'),
    h('h3', {}, 'Price of a meal'),
    sparkline(hist.map((d) => d.food_price), (v) => money(v), '#ffb74d'),
    h('h3', {}, 'Town treasury'),
    sparkline(hist.map((d) => d.treasury), (v) => money(v, 0), '#81c784'),
    h('h3', {}, 'Output (firms + public wages)'),
    sparkline(hist.map((d) => d.output), (v) => money(v, 0), '#64b5f6'),
  ];
}

function pyramid(bands: [number, number][]): HTMLElement {
  const max = Math.max(1, ...bands.flatMap(([m, f]) => [m, f]));
  const rows = bands
    .map(([m, f], i) => {
      const label = i === bands.length - 1 ? `${i * 5}+` : `${i * 5}–${i * 5 + 4}`;
      return h(
        'div',
        { class: 'pyr-row' },
        h('div', { class: 'pyr-bar m' }, h('div', { style: `width:${(m / max) * 100}%` })),
        h('span', { class: 'pyr-age' }, label),
        h('div', { class: 'pyr-bar f' }, h('div', { style: `width:${(f / max) * 100}%` })),
      );
    })
    .reverse();
  return h('div', { class: 'pyramid' }, h('div', { class: 'pyr-legend' }, h('span', {}, '♂ men'), h('span', {}, 'women ♀')), ...rows);
}

function demographySection(d: DemographyStats): HTMLElement[] {
  return [
    h('h3', {}, 'People'),
    h(
      'div',
      { class: 'stats-grid' },
      stat('average age', d.average_age),
      stat('couples', d.couples),
      stat('births (30 days)', d.births_30d),
      stat('deaths (30 days)', d.deaths_30d),
      stat('weddings (30 days)', d.marriages_30d),
      stat('expecting a baby', d.pregnant),
      stat('arrived (30 days)', d.arrivals_30d),
      stat('left (30 days)', d.departures_30d),
      stat('ill right now', d.ill),
      stat('university graduates', d.university_graduates),
      stat('commute out', d.commuters_out),
      stat('commute in', d.commuters_in),
    ),
    h('h3', {}, 'Population pyramid'),
    pyramid(d.pyramid),
    h('h3', {}, 'Population (60 days)'),
    sparkline(d.history.map((x) => x.population), (v) => v.toLocaleString('en-US'), '#ba68c8'),
  ];
}
