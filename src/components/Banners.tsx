import { dismissBanner, useStore } from "../lib/store";
import type { Banner } from "../lib/store";
import { Icon } from "./Icon";

function AuthBanner({ banner }: { banner: Extract<Banner, { kind: "auth" }> }) {
  const { event } = banner;
  const denied = typeof event.outcome === "object" ? event.outcome.denied : null;
  const ok = event.outcome === "authorized";
  return (
    <div className={`banner ${denied ? "error" : ok ? "ok" : ""}`} role="status">
      <span className="lead">
        <Icon name={denied ? "warning" : ok ? "check" : "info"} />
      </span>
      <span className="msg">
        {denied ? (
          <>
            <strong>Authorization was not granted.</strong> {event.provider} returned “{denied.error}”
            {denied.description ? ` — ${denied.description}` : ""}.
          </>
        ) : ok ? (
          <>
            <strong>Authorization completed.</strong> {event.provider} approved the request; the result goes to {event.deliveredTo}.
          </>
        ) : (
          <>
            <strong>Returned from {event.provider}.</strong> The sign-in page redirected to {event.deliveredTo}.
          </>
        )}
      </span>
      <button className="icon-btn small" aria-label="Dismiss" onClick={() => dismissBanner(banner.id)}>
        <Icon name="close" size={14} />
      </button>
    </div>
  );
}

export function Banners() {
  const banners = useStore((s) => s.banners);
  return (
    <>
      {banners.map((b) =>
        b.kind === "auth" ? (
          <AuthBanner key={b.id} banner={b} />
        ) : (
          <div key={b.id} className={`banner ${b.level === "error" ? "error" : ""}`} role={b.level === "error" ? "alert" : "status"}>
            <span className="lead">
              <Icon name={b.level === "error" ? "warning" : "info"} />
            </span>
            <span className="msg">{b.message}</span>
            <button className="icon-btn small" aria-label="Dismiss" onClick={() => dismissBanner(b.id)}>
              <Icon name="close" size={14} />
            </button>
          </div>
        ),
      )}
    </>
  );
}
