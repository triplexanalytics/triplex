import LockIcon from "./LockIcon";
import { PRO_INFO_URL, TIERS } from "./TierPicker.const";

/** Tier 1 (masking) is the default view; Tiers 2 and 3 open the dialog of the generator
 * extension for that tier. `disabledReasons` maps a tier to why it cannot be used right now (no
 * file loaded); a tier without an entry is enabled. A tier listed in `proTiers` has no extension
 * in this build: it is shown as a link to where Triplex Pro, which adds it, is described. */
export default function TierPicker({
  activeTier = 1,
  onSelectTier,
  disabledReasons = {},
  proTiers = [],
}) {
  const head = (tier) => (
    <>
      <span className="tier-head">
        <LockIcon strength={tier.lockStrength} />
        <span className="tier-label">{tier.label}</span>
      </span>
      <span className="tier-caption">{tier.caption}</span>
    </>
  );
  return (
    <div className="tier-picker">
      {TIERS.map((tier) =>
        proTiers.includes(tier.id) ? (
          <a
            key={tier.id}
            className="tier-option tier-option-pro"
            href={PRO_INFO_URL}
            target="_blank"
            rel="noopener noreferrer"
            title="Part of Triplex Pro, a paid extension to this app. Opens triplexanalytics.io in a new tab."
          >
            {head(tier)}
            <span className="tier-pro">Part of Triplex Pro</span>
          </a>
        ) : (
          <button
            key={tier.id}
            type="button"
            className={`tier-option${tier.id === activeTier ? " active" : ""}`}
            disabled={!!disabledReasons[tier.id]}
            aria-pressed={tier.id === activeTier}
            onClick={() => onSelectTier?.(tier.id)}
            title={disabledReasons[tier.id] ?? `${tier.caption}. ${tier.reversibility}.`}
          >
            {head(tier)}
            <span className={`tier-reversibility strength-${tier.lockStrength}`}>
              {tier.reversibility}
            </span>
          </button>
        ),
      )}
    </div>
  );
}
