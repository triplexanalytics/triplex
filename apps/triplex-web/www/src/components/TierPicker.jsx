import LockIcon from "./LockIcon";
import { TIERS } from "./TierPicker.const";

export default function TierPicker() {
  return (
    <div className="tier-picker">
      {TIERS.map((tier) => (
        <button
          key={tier.id}
          type="button"
          className={`tier-option${tier.id === 1 ? " active" : ""}`}
          disabled={tier.id !== 1}
          title={`${tier.caption}. ${tier.reversibility}.`}
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
