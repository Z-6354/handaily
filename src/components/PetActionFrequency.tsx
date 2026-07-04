interface Props {
  randomMinSec: number;
  randomMaxSec: number;
  randomAnimations: string[];
  busy: boolean;
  onPatch: (patch: { randomMinSec?: number; randomMaxSec?: number }) => void;
}

function formatSecLabel(sec: number): string {
  if (sec >= 60 && sec % 60 === 0) return `${sec / 60} 分钟`;
  if (sec >= 60) return `${(sec / 60).toFixed(1).replace(/\.0$/, "")} 分钟`;
  return `${sec} 秒`;
}

function formatFrequencyHint(minSec: number, maxSec: number): string {
  const min = Math.max(5, minSec);
  const max = Math.max(min, maxSec);
  if (min === max) return `约每 ${formatSecLabel(min)} 播放一次`;
  return `约 ${formatSecLabel(min)} ~ ${formatSecLabel(max)} 之间随机播放`;
}

const FREQUENCY_PRESETS = [
  { label: "频繁", min: 15, max: 45 },
  { label: "默认", min: 30, max: 120 },
  { label: "悠闲", min: 60, max: 300 },
] as const;

export function PetActionFrequency({
  randomMinSec,
  randomMaxSec,
  randomAnimations,
  busy,
  onPatch,
}: Props) {
  return (
    <div className="pet-action-frequency pet-action-frequency--inline">
      <div className="pet-action-frequency-head">
        <div>
          <div className="pet-action-frequency-label">随机动作频率</div>
          <p className="pet-action-frequency-hint">
            {randomAnimations.length === 0
              ? "请在「动作分配」中勾选随机动作"
              : formatFrequencyHint(randomMinSec, randomMaxSec)}
          </p>
        </div>
        <div className="pet-action-frequency-presets" role="group" aria-label="频率预设">
          {FREQUENCY_PRESETS.map((preset) => {
            const active = randomMinSec === preset.min && randomMaxSec === preset.max;
            return (
              <button
                key={preset.label}
                type="button"
                className={`pet-action-frequency-preset${active ? " is-active" : ""}`}
                disabled={busy || randomAnimations.length === 0}
                onClick={() => onPatch({ randomMinSec: preset.min, randomMaxSec: preset.max })}
              >
                {preset.label}
              </button>
            );
          })}
        </div>
      </div>
      <div
        className={`pet-action-frequency-range${
          randomAnimations.length === 0 ? " is-disabled" : ""
        }`}
      >
        <label className="pet-action-frequency-field">
          <span className="pet-action-frequency-field-label">最短间隔</span>
          <div className="pet-action-frequency-input-wrap">
            <input
              type="number"
              className="pet-action-frequency-input"
              min={5}
              max={3600}
              value={randomMinSec}
              disabled={busy || randomAnimations.length === 0}
              onChange={(e) => {
                const v = parseInt(e.target.value, 10) || 30;
                onPatch({ randomMinSec: v, randomMaxSec: Math.max(v, randomMaxSec) });
              }}
            />
            <span className="pet-action-frequency-unit">秒</span>
          </div>
        </label>
        <span className="pet-action-frequency-sep" aria-hidden>
          —
        </span>
        <label className="pet-action-frequency-field">
          <span className="pet-action-frequency-field-label">最长间隔</span>
          <div className="pet-action-frequency-input-wrap">
            <input
              type="number"
              className="pet-action-frequency-input"
              min={5}
              max={7200}
              value={randomMaxSec}
              disabled={busy || randomAnimations.length === 0}
              onChange={(e) => {
                const v = parseInt(e.target.value, 10) || 120;
                onPatch({ randomMaxSec: v, randomMinSec: Math.min(randomMinSec, v) });
              }}
            />
            <span className="pet-action-frequency-unit">秒</span>
          </div>
        </label>
      </div>
    </div>
  );
}
