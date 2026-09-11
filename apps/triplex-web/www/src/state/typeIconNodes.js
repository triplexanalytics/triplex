import { __iconNode as hash } from "lucide-react/dist/esm/icons/hash.mjs";
import { __iconNode as sigma } from "lucide-react/dist/esm/icons/sigma.mjs";
import { __iconNode as toggleLeft } from "lucide-react/dist/esm/icons/toggle-left.mjs";
import { __iconNode as type } from "lucide-react/dist/esm/icons/type.mjs";
import { __iconNode as scanBarcode } from "lucide-react/dist/esm/icons/scan-barcode.mjs";
import { __iconNode as landmark } from "lucide-react/dist/esm/icons/landmark.mjs";
import { __iconNode as building2 } from "lucide-react/dist/esm/icons/building-2.mjs";
import { __iconNode as mail } from "lucide-react/dist/esm/icons/mail.mjs";
import { __iconNode as phone } from "lucide-react/dist/esm/icons/phone.mjs";
import { __iconNode as creditCard } from "lucide-react/dist/esm/icons/credit-card.mjs";
import { __iconNode as network } from "lucide-react/dist/esm/icons/network.mjs";
import { __iconNode as globe } from "lucide-react/dist/esm/icons/globe.mjs";
import { __iconNode as idCard } from "lucide-react/dist/esm/icons/id-card.mjs";
import { __iconNode as calendar } from "lucide-react/dist/esm/icons/calendar.mjs";
import { __iconNode as keyRound } from "lucide-react/dist/esm/icons/key-round.mjs";
// "home.mjs" is just an alias re-exporting "house.mjs"'s default (Home ->
// House rename) — it doesn't forward __iconNode, so import the real file.
import { __iconNode as home } from "lucide-react/dist/esm/icons/house.mjs";
import { __iconNode as mapPin } from "lucide-react/dist/esm/icons/map-pin.mjs";

// Raw icon geometry for canvas rendering — TableTab's header is drawn via
// ctx.fillText/fillRect (Glide Data Grid's own canvas, not DOM), so the
// React <Icon/> components ColumnCard uses (typeIcons.jsx) can't be
// mounted there. lucide-react's public API only exports finished React
// components, not the underlying primitive-shape array each one is built
// from, so this reaches into each icon's own file for its `__iconNode`
// export instead of adding a second package (`lucide`) that would just
// duplicate the same geometry under a different name. package.json pins an
// exact lucide-react version specifically because this reaches past its
// public API.
//
// Keep in sync with typeIcons.jsx's ICONS map — same 18 keys, same choices.
export const TYPE_ICON_NODES = {
  integer: hash,
  float: sigma,
  boolean: toggleLeft,
  string: type,
  numeric_id: scanBarcode,
  iban: landmark,
  bic: building2,
  email: mail,
  phone,
  credit_card: creditCard,
  mac_address: network,
  ip_address: globe,
  national_id: idCard,
  date_of_birth: calendar,
  credential_secret: keyRound,
  address: home,
  location: mapPin,
  likely_identifier: scanBarcode,
};

/** Draws a lucide icon node array (`[[tag, attrs], ...]`, a 24x24-viewBox
 * stroke icon) onto a canvas 2D context at `(x, y)` — top-left corner —
 * scaled to `size` pixels square. Generic over every lucide icon's shape
 * primitives (path/line/circle/rect/polyline/polygon/ellipse), not just
 * the ones this app happens to use today. */
export function drawIconNode(ctx, iconNode, x, y, size, color) {
  ctx.save();
  ctx.translate(x, y);
  ctx.scale(size / 24, size / 24);
  ctx.strokeStyle = color;
  ctx.lineWidth = 2;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  for (const [tag, attrs] of iconNode) {
    if (tag === "path") {
      ctx.stroke(new Path2D(attrs.d));
      continue;
    }
    ctx.beginPath();
    switch (tag) {
      case "line":
        ctx.moveTo(+attrs.x1, +attrs.y1);
        ctx.lineTo(+attrs.x2, +attrs.y2);
        break;
      case "circle":
        ctx.arc(+attrs.cx, +attrs.cy, +attrs.r, 0, Math.PI * 2);
        break;
      case "rect": {
        const rx = +(attrs.rx ?? attrs.ry ?? 0);
        if (rx > 0 && ctx.roundRect) {
          ctx.roundRect(+attrs.x, +attrs.y, +attrs.width, +attrs.height, rx);
        } else {
          ctx.rect(+attrs.x, +attrs.y, +attrs.width, +attrs.height);
        }
        break;
      }
      case "polyline":
      case "polygon": {
        const points = attrs.points
          .trim()
          .split(/\s+/)
          .map((p) => p.split(",").map(Number));
        points.forEach(([px, py], i) => (i === 0 ? ctx.moveTo(px, py) : ctx.lineTo(px, py)));
        if (tag === "polygon") ctx.closePath();
        break;
      }
      case "ellipse":
        ctx.ellipse(+attrs.cx, +attrs.cy, +attrs.rx, +attrs.ry, 0, 0, Math.PI * 2);
        break;
      default:
        continue;
    }
    ctx.stroke();
  }
  ctx.restore();
}
