// Renderer, isometric orthographic camera, controls and the day/night cycle.

import * as THREE from 'three';
import { daylight } from '../sim/time';

export class Stage {
  readonly renderer: THREE.WebGLRenderer;
  readonly scene = new THREE.Scene();
  readonly camera: THREE.OrthographicCamera;
  readonly target = new THREE.Vector3();
  readonly sun = new THREE.DirectionalLight(0xffffff, 2.2);
  readonly hemi = new THREE.HemisphereLight(0xdfefff, 0x6b8f4e, 1.2);
  /** World units visible vertically at zoom 1. */
  private readonly viewSize = 60;
  private readonly offset = new THREE.Vector3(1, 1.15, 1).normalize().multiplyScalar(3000);
  private readonly daySky = new THREE.Color(0x9fd8ef);
  private readonly duskSky = new THREE.Color(0xf2a37b);
  private readonly nightSky = new THREE.Color(0x0e1a33);
  /** 0 = day … 1 = night (shared with materials). */
  readonly night = { value: 0 };
  onUserMove: () => void = () => {};

  constructor(container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: window.devicePixelRatio < 2, powerPreference: 'high-performance' });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.shadowMap.enabled = false;
    container.appendChild(this.renderer.domElement);
    this.camera = new THREE.OrthographicCamera(-1, 1, 1, -1, 1, 9000);
    this.camera.zoom = 1.1;
    this.scene.add(this.hemi, this.sun, this.sun.target);
    this.scene.background = this.daySky.clone();
    this.resize();
    window.addEventListener('resize', () => this.resize());
    this.installControls();
  }

  resize() {
    const w = this.renderer.domElement.parentElement!.clientWidth;
    const h = this.renderer.domElement.parentElement!.clientHeight;
    this.renderer.setSize(w, h);
    const aspect = w / h;
    this.camera.left = (-this.viewSize * aspect) / 2;
    this.camera.right = (this.viewSize * aspect) / 2;
    this.camera.top = this.viewSize / 2;
    this.camera.bottom = -this.viewSize / 2;
    this.camera.updateProjectionMatrix();
  }

  lookAt(x: number, z: number) {
    this.target.set(x, 0, z);
  }

  updateCamera() {
    this.camera.position.copy(this.target).add(this.offset);
    this.camera.lookAt(this.target);
    this.camera.updateMatrixWorld();
  }

  /** World units per screen pixel. */
  unitsPerPixel(): number {
    return this.viewSize / this.camera.zoom / this.renderer.domElement.clientHeight;
  }

  setTime(tick: number) {
    const d = daylight(tick);
    this.night.value = 1 - d;
    const sky = this.nightSky.clone().lerp(this.duskSky, Math.min(1, d * 2)).lerp(this.daySky, Math.max(0, d * 2 - 1));
    (this.scene.background as THREE.Color).copy(sky);
    this.sun.intensity = 0.15 + 2.1 * d;
    this.hemi.intensity = 0.35 + 0.9 * d;
    this.hemi.color.setRGB(0.55 + 0.35 * d, 0.6 + 0.33 * d, 0.85 + 0.15 * d);
    const angle = ((tick % 1440) / 1440) * Math.PI * 2;
    this.sun.position.set(this.target.x + Math.cos(angle) * 100, 160, this.target.z + 80 + Math.sin(angle) * 40);
    this.sun.target.position.copy(this.target);
  }

  render() {
    this.updateCamera();
    this.renderer.render(this.scene, this.camera);
  }

  private installControls() {
    const el = this.renderer.domElement;
    const pointers = new Map<number, { x: number; y: number }>();
    let pinchDist = 0;
    let moved = 0;
    const right = new THREE.Vector3();
    const fwd = new THREE.Vector3();
    const pan = (dx: number, dy: number) => {
      const upp = this.unitsPerPixel();
      right.set(1, 0, -1).normalize();
      fwd.set(-1, 0, -1).normalize();
      // Screen-up maps to "into the screen" on the ground, stretched by the view angle.
      const k = 1 / Math.sin(Math.atan2(this.offset.y, Math.hypot(this.offset.x, this.offset.z)));
      this.target.addScaledVector(right, -dx * upp).addScaledVector(fwd, dy * upp * k);
      this.onUserMove();
    };
    el.addEventListener('pointerdown', (e) => {
      el.setPointerCapture(e.pointerId);
      pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
      moved = 0;
      if (pointers.size === 2) {
        const [a, b] = [...pointers.values()];
        pinchDist = Math.hypot(a.x - b.x, a.y - b.y);
      }
    });
    el.addEventListener('pointermove', (e) => {
      const p = pointers.get(e.pointerId);
      if (!p) return;
      if (pointers.size === 1) {
        pan(e.clientX - p.x, e.clientY - p.y);
        moved += Math.abs(e.clientX - p.x) + Math.abs(e.clientY - p.y);
      }
      p.x = e.clientX;
      p.y = e.clientY;
      if (pointers.size === 2) {
        const [a, b] = [...pointers.values()];
        const d = Math.hypot(a.x - b.x, a.y - b.y);
        if (pinchDist > 0) this.zoomBy(d / pinchDist);
        pinchDist = d;
        moved += 10;
      }
    });
    const up = (e: PointerEvent) => {
      pointers.delete(e.pointerId);
      if (pointers.size < 2) pinchDist = 0;
    };
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
    el.addEventListener(
      'wheel',
      (e) => {
        e.preventDefault();
        this.zoomBy(Math.exp(-e.deltaY * 0.0015));
      },
      { passive: false },
    );
    window.addEventListener('keydown', (e) => {
      if ((e.target as HTMLElement)?.tagName === 'INPUT') return;
      const step = 40;
      if (e.key === 'ArrowLeft' || e.key === 'a') pan(step, 0);
      if (e.key === 'ArrowRight' || e.key === 'd') pan(-step, 0);
      if (e.key === 'ArrowUp' || e.key === 'w') pan(0, step);
      if (e.key === 'ArrowDown' || e.key === 's') pan(0, -step);
      if (e.key === '+' || e.key === '=') this.zoomBy(1.2);
      if (e.key === '-') this.zoomBy(1 / 1.2);
    });
    this.wasDrag = () => moved > 6;
  }

  wasDrag: () => boolean = () => false;

  zoomBy(f: number) {
    this.camera.zoom = Math.min(16, Math.max(0.08, this.camera.zoom * f));
    this.camera.updateProjectionMatrix();
  }
}
