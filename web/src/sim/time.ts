// Simulated time, mirroring livingsim-core/src/time.rs.
// Tick 0 = Year 1, Monday January 1st, 06:00. 1 tick = 1 simulated minute = 2.5 real s.

export const MS_PER_TICK = 2500;
export const TICKS_PER_DAY = 1440;
const START_MINUTE = 360;
const MONTH_DAYS = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
export const MONTHS = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December'];
export const WEEKDAYS = ['Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'];

/** Fractional tick for a real time. */
export function tickAt(nowMs: number, epochMs: number): number {
  return Math.max(0, (nowMs - epochMs) / MS_PER_TICK);
}

export function minuteOfDay(t: number): number {
  return (t + START_MINUTE) % TICKS_PER_DAY;
}

export interface SimDate {
  year: number;
  month: number;
  day: number;
  weekday: number;
  hour: number;
  minute: number;
}

export function date(t: number): SimDate {
  const abs = Math.floor(t) + START_MINUTE;
  const dayNum = Math.floor(abs / TICKS_PER_DAY);
  const year = Math.floor(dayNum / 365) + 1;
  let doy = dayNum % 365;
  let month = 0;
  while (doy >= MONTH_DAYS[month]) {
    doy -= MONTH_DAYS[month];
    month++;
  }
  const m = abs % TICKS_PER_DAY;
  return { year, month: month + 1, day: doy + 1, weekday: dayNum % 7, hour: Math.floor(m / 60), minute: m % 60 };
}

export function formatClock(t: number): string {
  const d = date(t);
  return `${String(d.hour).padStart(2, '0')}:${String(d.minute).padStart(2, '0')}`;
}

export function formatDate(t: number): string {
  const d = date(t);
  return `${WEEKDAYS[d.weekday]}, ${MONTHS[d.month - 1]} ${d.day}, Year ${d.year}`;
}

/** 0 at midnight … 1 at noon, smooth daylight factor with dawn/dusk. */
export function daylight(t: number): number {
  const h = minuteOfDay(t) / 60;
  const sunrise = 6.5;
  const sunset = 20.25;
  if (h < sunrise - 1 || h > sunset + 1) return 0;
  if (h < sunrise + 1) return smooth((h - (sunrise - 1)) / 2);
  if (h > sunset - 1) return smooth(((sunset + 1) - h) / 2);
  return 1;
}

function smooth(x: number): number {
  const c = Math.min(1, Math.max(0, x));
  return c * c * (3 - 2 * c);
}
