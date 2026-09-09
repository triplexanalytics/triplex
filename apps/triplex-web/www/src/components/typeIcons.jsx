import {
  Hash,
  Sigma,
  ToggleLeft,
  Type,
  ScanBarcode,
  Landmark,
  Building2,
  Mail,
  Phone,
  CreditCard,
  Network,
  Globe,
  IdCard,
  Calendar,
  KeyRound,
  Home,
  MapPin,
} from "lucide-react";

// One icon per primitive ColumnType (crates/triplex-schemacast/src/lib.rs)
// and per SemanticType (crates/triplex-schemacast/src/semantic.rs) — kept
// as one lookup table so a new backend variant is a single line to add
// here, not a hunt through JSX for every place a type renders.
const ICONS = {
  // ColumnType
  integer: Hash,
  float: Sigma,
  boolean: ToggleLeft,
  string: Type,
  // SemanticType
  numeric_id: ScanBarcode,
  iban: Landmark,
  bic: Building2,
  email: Mail,
  phone: Phone,
  credit_card: CreditCard,
  mac_address: Network,
  ip_address: Globe,
  national_id: IdCard,
  date_of_birth: Calendar,
  credential_secret: KeyRound,
  address: Home,
  location: MapPin,
};

/** Renders the icon for a `ColumnType` or `SemanticType` value (both are
 * plain snake_case strings over the wasm/JSON boundary), or nothing for a
 * type this lookup doesn't know yet — a missing icon should never break
 * rendering the type's name next to it. */
export default function TypeIcon({ type, ...props }) {
  const Icon = ICONS[type];
  if (!Icon) return null;
  return <Icon size={14} strokeWidth={2} aria-hidden="true" {...props} />;
}
