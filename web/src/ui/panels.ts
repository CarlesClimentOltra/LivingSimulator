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
}

export interface BuildingCard {
  id: number;
  name: string;
  kind: string;
  occupants: { id: number; name: string }[];
  present: number;
  stock: number;
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

export function money(cents: number): string {
  const v = (cents / 100).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  return `${v} ₡`;
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
    ),
    h('h3', {}, 'Needs'),
    h('div', { class: 'needs' }, ...c.needs.map(([n, v]) => bar(n, v))),
    h('h3', {}, 'Family'),
    fam,
    h('h3', {}, 'Diary'),
    h('ol', { class: 'journal' }, ...c.journal.map((j) => h('li', {}, h('span', { class: 'when' }, j.when), ' ', j.text))),
  );
}

export function renderBuildingCard(el: HTMLElement, b: BuildingCard, onPerson: (id: number) => void) {
  el.replaceChildren(
    ...([
    h('div', { class: 'card-head' }, h('div', { class: 'avatar b' }, '⌂'), h('div', {}, h('h2', {}, b.name), h('div', { class: 'muted' }, `${b.kind} · ${b.present} inside`))),
    b.stock > 0 ? h('p', {}, `Stock: ${b.stock} meals`) : null,
    h('h3', {}, 'Inside now'),
    b.occupants.length
      ? h(
          'ul',
          { class: 'family' },
          ...b.occupants.map((o) => {
            const a = h('a', { href: '#' }, o.name);
            a.addEventListener('click', (e) => {
              e.preventDefault();
              onPerson(o.id);
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
      stat('wages paid', money(s.wages_yesterday_cents)),
      stat('avg savings', money(s.avg_money_cents)),
    ),
  );
}

export function renderNews(el: HTMLElement, cityName: string, news: NewsLine[]) {
  el.replaceChildren(
    h('div', { class: 'paper-title' }, `The ${cityName} Chronicle`),
    ...news.slice(0, 20).map((n) => h('article', { class: 'news' }, h('div', { class: 'when' }, n.when), h('h4', {}, n.headline), h('p', {}, n.body))),
  );
}
