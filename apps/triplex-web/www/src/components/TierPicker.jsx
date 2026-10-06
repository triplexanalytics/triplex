import LockIcon from "./LockIcon";
import { TIERS } from "./TierPicker.const";

/** Tier 1 (masking) is the default view; Tier 2 (synthetic data) opens the synthesis dialog, and is
 * enabled once a file is loaded. Tier 3 is still a "coming soon" placeholder. */
export default function TierPicker({ activeTier = 1, onSelectTier, tier2Enabled = false }) {
  const isEnabled = (id) => id === 1 || (id === 2 && tier2Enabled);
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
          title={
            tier.id === 2 && !tier2Enabled
              ? "Load a file to generate synthetic data"
              : `${tier.caption}. ${tier.reversibility}.`
          }
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
