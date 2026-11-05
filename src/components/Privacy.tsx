import { Icon, type IconName } from "./Icon";

// Each point is taken from the README's security model and the Rust sources (security/, commands/).
const COMMITMENTS: { icon: IconName; title: string; body: string }[] = [
  {
    icon: "shield",
    title: "Websites can’t talk to Zuno",
    body: "Website tabs and popups get no app permissions, and every Zuno command checks that it came from Zuno’s own interface. Calls from a page are refused.",
  },
  {
    icon: "lock",
    title: "Certificate errors stay errors",
    body: "HTTPS validation is left entirely to WebView2. There is no button to click through a bad certificate.",
  },
  {
    icon: "history",
    title: "History keeps no secrets",
    body: "Sign-in codes, access tokens and passwords are stripped from addresses before a visit is saved, and the importer applies the same cleaning.",
  },
  {
    icon: "link",
    title: "Links that open apps ask first",
    body: "javascript:, data: and privileged Windows schemes are blocked. mailto: links and app callbacks open only after you confirm.",
  },
  {
    icon: "key",
    title: "Tokens live in Credential Manager",
    body: "Accounts you connect use the authorization code flow with PKCE. Their tokens are stored in Windows Credential Manager, never in plain files or logs.",
  },
  {
    icon: "file",
    title: "Downloads that can’t surprise you",
    body: "File names are sanitised against path tricks, existing files are never overwritten, and Zuno only opens files its own download manager saved.",
  },
];

const LIMITS = [
  {
    title: "Cookies and passwords from Chromium browsers aren’t imported",
    body: "Chrome, Edge and similar browsers encrypt them with keys tied to that browser. Zuno doesn’t work around another app’s protection.",
  },
  {
    title: "Incognito isn’t invisibility",
    body: "Websites, your network operator and anyone watching the network can still see what you visit, and downloaded files stay on disk.",
  },
  {
    title: "One profile per window",
    body: "All tabs in a window share one browsing profile, just as they do in a regular browser window.",
  },
  {
    title: "Windows only",
    body: "Zuno is built on WebView2 and Windows APIs, from Credential Manager to native menus.",
  },
];

// The tab-group tones, one per row.
const TONES = ["#a59ad0", "#86a1ad", "#8fa58f", "#b5a383", "#b48a7c", "#a28aa6"];

export function Privacy() {
  return (
    <section className="section privacy" id="privacy" aria-labelledby="privacy-title">
      <div className="container privacy__grid">
        <header className="privacy__head">
          <h2 id="privacy-title" className="display-2">
            Quiet on the surface. Strict underneath.
          </h2>
          <p className="section__lede">
            A calm interface doesn’t mean loose rules. These checks run in the Rust core, where a website can’t reach them.
          </p>
        </header>

        <ul className="ledger">
          {COMMITMENTS.map((c, i) => (
            <li key={c.title} className="ledger__row" data-reveal style={{ "--tone": TONES[i % TONES.length] } as React.CSSProperties}>
              <span className="ledger__icon">
                <Icon name={c.icon} size={18} />
              </span>
              <h3>{c.title}</h3>
              <p>{c.body}</p>
            </li>
          ))}
        </ul>
      </div>

      <div className="container">
        <aside className="fineprint" aria-labelledby="fineprint-title" data-reveal>
          <h3 id="fineprint-title">
            The fine print <span>(it’s short, read it)</span>
          </h3>
          <ul>
            {LIMITS.map((l) => (
              <li key={l.title}>
                <strong>{l.title}.</strong> {l.body}
              </li>
            ))}
          </ul>
        </aside>
      </div>
    </section>
  );
}
