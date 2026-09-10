import { t } from "../../i18n";
import { useAppState, useInstances, useModules, useSelectedAccount } from "../../store/AppStore";
import { Badge } from "../../components/ui/primitives";
import { LinksBar } from "../links/LinksBar";
import { GameConsole } from "./GameConsole";
import { NewsFeed } from "./NewsFeed";
import { PlayCard } from "./PlayCard";
import { ServerStatusCard } from "./ServerStatusCard";

export function HomeView() {
  const account = useSelectedAccount();
  const { selected } = useInstances();
  const { game } = useAppState();
  const enabled = useModules();

  return (
    <div className="space-y-6">
      <section
        className="relative overflow-hidden rounded-2xl p-6 sm:p-8 animate-fade-in"
        style={{
          border: "1px solid color-mix(in srgb, var(--border) 60%, transparent)",
          background: "linear-gradient(to bottom right, var(--bg-primary), color-mix(in srgb, var(--bg-primary) 70%, var(--bg-deep)), var(--bg-deep))",
        }}
      >
        <div className="blob -top-24 -right-16 w-80 h-80" style={{ background: "rgba(34,197,94,0.10)" }} />
        <div className="blob top-1/2 right-1/3 w-56 h-56" style={{ background: "rgba(34,211,238,0.08)" }} />
        <div className="relative flex flex-col lg:flex-row lg:items-end lg:justify-between gap-5">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2 mb-3">
              {game.running ? (
                <Badge variant="brand" dot pulse>
                  {t("home.running")}
                </Badge>
              ) : null}
              {selected ? <Badge variant="neutral">{selected.name}</Badge> : null}
            </div>
            <h1 className="font-display text-2xl sm:text-3xl lg:text-4xl font-bold tracking-tight" style={{ color: "var(--text-primary)" }}>
              {account ? (
                <>
                  {t("home.greeting")} <span className="text-gradient">{account.name}</span>
                </>
              ) : (
                t("home.greetingGuest")
              )}
            </h1>
            <p className="text-sm mt-2 max-w-xl" style={{ color: "var(--text-body)" }}>
              {account ? t("home.subtitle") : t("home.subtitleNoAccount")}
            </p>
          </div>
          {enabled("links") ? <LinksBar compact /> : null}
        </div>
      </section>

      <div className="grid grid-cols-1 xl:grid-cols-[minmax(0,1fr)_320px] gap-5 items-start">
        <div className="space-y-5 min-w-0">
          <PlayCard />
          <GameConsole />
          {enabled("news") ? <NewsFeed /> : null}
        </div>
        <div className="space-y-5">{enabled("serverStatus") ? <ServerStatusCard instance={selected} /> : null}</div>
      </div>
    </div>
  );
}
