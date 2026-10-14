import LockIcon from "./LockIcon";
import { TIERS } from "./TierPicker.const";

/** Tier 1 (masking) is the default view; Tiers 2 and 3 open the dialog of the generator
 * extension for that tier. `disabledReasons` maps a tier to why it cannot be used right now (no
 * file loaded, no extension for it in this build); a tier without an entry is enabled. */
export default function TierPicker({ activeTier = 1, onSelectTier, disabledReasons = {} }) {
  const isEnabled = (id) => !disabledReasons[id];
  return (
    <div className="tier-picker">
      {TIERS.map((tier) => (
        <button
          key={tier.id}
          type="button"
          className={`tier-option${tier.id === activeTier ? " active" : ""}`}
          disabled={!isEnabled(tier.id)}
          aria-pressed={tier.id === activeTier}
          onClick={() => onSelectTier?.(tier.id)}
          title={disabledReasons[tier.id] ?? `${tier.caption}. ${tier.reversibility}.`}
        >
          <span className="tier-head">
            <LockIcon strength={tier.lockStrength} />
            <span className="tier-label">{tier.label}</span>
          </span>
          <span className="tier-caption">{tier.caption}</span>
          <span className={`tier-reversibility strength-${tier.lockStrength}`}>
            {tier.reversibility}
          </span>
        </button>
      ))}
    </div>
  );
}
