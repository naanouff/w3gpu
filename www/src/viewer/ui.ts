import { type LivePhaseA, toPhaseAJson } from './phaseAConfig.js';

export type LightParams = {
  dirX: number;
  dirY: number;
  dirZ: number;
  colorR: number;
  colorG: number;
  colorB: number;
  directionalIntensity: number;
  ambient: number;
  shadowBias: number;
};

export const DEFAULT_LIGHT: LightParams = {
  dirX: -0.5,
  dirY: -1.0,
  dirZ: -0.5,
  colorR: 1.0,
  colorG: 0.95,
  colorB: 0.9,
  directionalIntensity: 1.0,
  ambient: 0.12,
  shadowBias: 0.001,
};

export type ViewerPanelCallbacks = {
  onLivePhaseA: (json: string) => void;
  onCull: (enabled: boolean) => void;
  onReframe: () => void;
  onLight: (l: LightParams) => void;
  /** Après changement de `ibl_tier` : recharger le HDR en mémoire si possible. */
  onIblTierChanged: () => void;
  onHdrFile: (file: File) => void;
  onGlbFile: (file: File) => void;
};

const IBL_TIERS = ['min', 'low', 'medium', 'high', 'max'] as const;

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className: string,
  text?: string,
): HTMLElementTagNameMap[K] {
  const n = document.createElement(tag);
  n.className = className;
  if (text !== undefined) {
    n.textContent = text;
  }
  return n;
}

/**
 * Panneau latéral (même grammaire de réglages que le viewer natif prévu).
 */
export function mountViewerPanel(
  parent: HTMLElement,
  initial: LivePhaseA,
  c: ViewerPanelCallbacks,
): void {
  let live: LivePhaseA = {
    ibl_diffuse_scale: initial.ibl_diffuse_scale,
    ibl_tier: initial.ibl_tier,
    tonemap: { ...initial.tonemap },
  };
  let bloomEnabled = live.tonemap.bloom_strength > 0.0;
  let bloomStored = live.tonemap.bloom_strength > 0.0 ? live.tonemap.bloom_strength : 0.1;
  const light: LightParams = { ...DEFAULT_LIGHT };

  const pushPhaseA = (): void => {
    c.onLivePhaseA(toPhaseAJson(live));
  };

  const root = el('div', 'w3d-panel');
  root.innerHTML = `
    <h2 class="w3d-panel__title">w3drs viewer</h2>
    <section class="w3d-section"><h3>Environnement</h3>
      <label class="w3d-row">HDR <input type="file" class="w3d-file" data-kind="hdr" accept=".hdr,.rgbe" /></label>
      <label class="w3d-row">GLB <input type="file" class="w3d-file" data-kind="glb" accept=".glb" /></label>
      <label class="w3d-row">IBL tier
        <select class="w3d-tier">${IBL_TIERS.map(
          (t) => `<option value="${t}" ${t === live.ibl_tier ? 'selected' : ''}>${t}</option>`,
        ).join('')}
        </select>
      </label>
      <label class="w3d-row">IBL diffuse <input class="w3d-range" data-k="ibl" type="range" min="0" max="2" step="0.01" value="${String(live.ibl_diffuse_scale)}" />
        <span class="w3d-val" data-v="ibl">${live.ibl_diffuse_scale.toFixed(2)}</span>
      </label>
      <p class="w3d-hint">Le tier n’a effet qu’au prochain chargement HDR (re-fichier ou re-tier → reload auto si HDR déjà en mémoire côté app).</p>
    </section>
    <section class="w3d-section"><h3>Image</h3>
      <label class="w3d-row">Exposition <input class="w3d-range" data-k="exp" type="range" min="0.1" max="4" step="0.01" value="${String(live.tonemap.exposure)}" />
        <span class="w3d-val" data-v="exp">${live.tonemap.exposure.toFixed(2)}</span>
      </label>
      <label class="w3d-row"><input type="checkbox" class="w3d-bloom-enabled" ${bloomEnabled ? 'checked' : ''} /> Bloom actif</label>
      <label class="w3d-row">Bloom <input class="w3d-range" data-k="bloom" type="range" min="0" max="1" step="0.01" value="${String(live.tonemap.bloom_strength)}" />
        <span class="w3d-val" data-v="bloom">${live.tonemap.bloom_strength.toFixed(2)}</span>
      </label>
      <label class="w3d-row"><input type="checkbox" class="w3d-fxaa" ${live.tonemap.fxaa ? 'checked' : ''} /> FXAA (post-tonemap)</label>
      <p class="w3d-hint">MSAA sur le pass HDR principal est choisi par l’adaptateur WebGPU (souvent 4× si supporté).</p>
    </section>
    <section class="w3d-section"><h3>Lumière</h3>
      <label class="w3d-row">dir X <input class="w3d-range" data-lk="dx" type="range" min="-1" max="1" step="0.01" value="${String(light.dirX)}" />
        <span class="w3d-val" data-lv="dx">${light.dirX.toFixed(2)}</span></label>
      <label class="w3d-row">dir Y <input class="w3d-range" data-lk="dy" type="range" min="-1" max="0" step="0.01" value="${String(light.dirY)}" />
        <span class="w3d-val" data-lv="dy">${light.dirY.toFixed(2)}</span></label>
      <label class="w3d-row">dir Z <input class="w3d-range" data-lk="dz" type="range" min="-1" max="1" step="0.01" value="${String(light.dirZ)}" />
        <span class="w3d-val" data-lv="dz">${light.dirZ.toFixed(2)}</span></label>
      <label class="w3d-row">R <input class="w3d-range" data-lk="cr" type="range" min="0" max="2" step="0.01" value="${String(light.colorR)}" />
        <span class="w3d-val" data-lv="cr">${light.colorR.toFixed(2)}</span></label>
      <label class="w3d-row">G <input class="w3d-range" data-lk="cg" type="range" min="0" max="2" step="0.01" value="${String(light.colorG)}" />
        <span class="w3d-val" data-lv="cg">${light.colorG.toFixed(2)}</span></label>
      <label class="w3d-row">B <input class="w3d-range" data-lk="cb" type="range" min="0" max="2" step="0.01" value="${String(light.colorB)}" />
        <span class="w3d-val" data-lv="cb">${light.colorB.toFixed(2)}</span></label>
      <label class="w3d-row">Int. directionnelle <input class="w3d-range" data-lk="di" type="range" min="0" max="3" step="0.01" value="${String(light.directionalIntensity)}" />
        <span class="w3d-val" data-lv="di">${light.directionalIntensity.toFixed(2)}</span></label>
      <label class="w3d-row">Ambiant <input class="w3d-range" data-lk="amb" type="range" min="0" max="0.6" step="0.001" value="${String(light.ambient)}" />
        <span class="w3d-val" data-lv="amb">${light.ambient.toFixed(3)}</span></label>
      <label class="w3d-row">Shadow bias <input class="w3d-range" data-lk="sb" type="range" min="0" max="0.01" step="0.0001" value="${String(light.shadowBias)}" />
        <span class="w3d-val" data-lv="sb">${light.shadowBias.toFixed(4)}</span></label>
    </section>
    <section class="w3d-section"><h3>Rendu</h3>
      <label class="w3d-row"><input type="checkbox" class="w3d-cull" />
        Culling Hi-Z (H, Build)</label>
      <p class="w3d-hint">Désactivé par défaut : le test Hi-Z par AABB peut cacher des morceaux «&nbsp;derrière&nbsp;» d’autres (ex. montre multi-mesh, vue du dessus).</p>
      <button type="button" class="w3d-btn w3d-reframe">Reframe camera (AABB)</button>
    </section>
  `;
  parent.appendChild(root);

  const syncLightFromDom = (): void => {
    const rs = root.querySelectorAll<HTMLInputElement>('.w3d-range[data-lk]');
    for (const inp of rs) {
      const k = inp.dataset.lk;
      if (k === 'dx') { light.dirX = parseFloat(inp.value); }
      else if (k === 'dy') { light.dirY = parseFloat(inp.value); }
      else if (k === 'dz') { light.dirZ = parseFloat(inp.value); }
      else if (k === 'cr') { light.colorR = parseFloat(inp.value); }
      else if (k === 'cg') { light.colorG = parseFloat(inp.value); }
      else if (k === 'cb') { light.colorB = parseFloat(inp.value); }
      else if (k === 'di') { light.directionalIntensity = parseFloat(inp.value); }
      else if (k === 'amb') { light.ambient = parseFloat(inp.value); }
      else if (k === 'sb') { light.shadowBias = parseFloat(inp.value); }
    }
    c.onLight(light);
  };

  root.querySelector('.w3d-file[data-kind="hdr"]')!.addEventListener('change', (ev) => {
    const f = (ev.target as HTMLInputElement).files?.[0];
    if (f) {
      c.onHdrFile(f);
    }
  });
  root.querySelector('.w3d-file[data-kind="glb"]')!.addEventListener('change', (ev) => {
    const f = (ev.target as HTMLInputElement).files?.[0];
    if (f) {
      c.onGlbFile(f);
    }
  });

  root.addEventListener('input', (ev) => {
    const t = ev.target as HTMLInputElement;
    if (t.classList.contains('w3d-range') && t.dataset.k === 'ibl') {
      const v = parseFloat(t.value);
      live = { ...live, ibl_diffuse_scale: v };
      root.querySelector(`[data-v="ibl"]`)!.textContent = v.toFixed(2);
      pushPhaseA();
    } else if (t.classList.contains('w3d-range') && t.dataset.k === 'exp') {
      const v = parseFloat(t.value);
      live = { ...live, tonemap: { ...live.tonemap, exposure: v } };
      root.querySelector(`[data-v="exp"]`)!.textContent = v.toFixed(2);
      pushPhaseA();
    } else if (t.classList.contains('w3d-range') && t.dataset.k === 'bloom') {
      const v = parseFloat(t.value);
      if (v > 0.0) {
        bloomStored = v;
      }
      live = { ...live, tonemap: { ...live.tonemap, bloom_strength: v } };
      root.querySelector(`[data-v="bloom"]`)!.textContent = v.toFixed(2);
      pushPhaseA();
    } else if (t.classList.contains('w3d-range') && t.dataset.lk) {
      syncLightFromDom();
      const lv = t.dataset.lk;
      const span = root.querySelector(`[data-lv="${lv}"]`);
      if (span) {
        const v = parseFloat(t.value);
        span.textContent = lv === 'amb' || lv === 'sb' ? v.toFixed(lv === 'amb' ? 3 : 4) : v.toFixed(2);
      }
    }
  });

  root.querySelector('.w3d-cull')!.addEventListener('change', (ev) => {
    c.onCull((ev.target as HTMLInputElement).checked);
  });
  root.querySelector('.w3d-bloom-enabled')!.addEventListener('change', (ev) => {
    bloomEnabled = (ev.target as HTMLInputElement).checked;
    const bloomInput = root.querySelector<HTMLInputElement>('.w3d-range[data-k="bloom"]')!;
    const next = bloomEnabled ? bloomStored : 0.0;
    bloomInput.value = String(next);
    live = { ...live, tonemap: { ...live.tonemap, bloom_strength: next } };
    root.querySelector(`[data-v="bloom"]`)!.textContent = next.toFixed(2);
    pushPhaseA();
  });
  root.querySelector('.w3d-fxaa')!.addEventListener('change', (ev) => {
    const on = (ev.target as HTMLInputElement).checked;
    live = { ...live, tonemap: { ...live.tonemap, fxaa: on } };
    pushPhaseA();
  });
  root.querySelector('.w3d-reframe')!.addEventListener('click', () => {
    c.onReframe();
  });

  root.querySelector('.w3d-tier')!.addEventListener('change', (ev) => {
    const sel = ev.target as HTMLSelectElement;
    live = { ...live, ibl_tier: sel.value };
    pushPhaseA();
    c.onIblTierChanged();
  });

  // Apply initial (ne pas forcer c.onCull(true) : la case reflète l’état, aligné sur le moteur défaut)
  pushPhaseA();
  syncLightFromDom();
  c.onCull(root.querySelector<HTMLInputElement>('.w3d-cull')!.checked);
}
