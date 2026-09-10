import { useEffect, useRef, useState } from "react";

import { t } from "../../i18n";
import { useActions, useAppState } from "../../store/AppStore";
import { Button, IconButton } from "../../components/ui/Button";
import { Icon } from "../../components/ui/Icon";
import { Card } from "../../components/ui/primitives";

/** Live (redacted) output of the Minecraft process. */
export function GameConsole() {
  const { game } = useAppState();
  const { clearLogs, openFolder } = useActions();
  const [open, setOpen] = useState(false);
  const [follow, setFollow] = useState(true);
  const list = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (open && follow && list.current) list.current.scrollTop = list.current.scrollHeight;
  }, [game.logs, open, follow]);

  if (game.logs.length === 0 && !game.running) return null;

  return (
    <Card static padding="p-0" className="overflow-hidden">
      <button type="button" className="w-full flex items-center gap-3 px-4 py-3 text-left" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        <Icon name="terminal" size={18} style={{ color: "#67e8f9" }} />
        <span className="flex-1 text-sm font-semibold" style={{ color: "var(--text-primary)" }}>
          {t("home.console")}
        </span>
        <span className="text-[11px] font-mono" style={{ color: "var(--text-meta)" }}>
          {game.logs.length}
        </span>
        <Icon name={open ? "expand_less" : "expand_more"} size={20} style={{ color: "var(--text-meta)" }} />
      </button>
      {open ? (
        <div className="px-4 pb-4 space-y-2">
          <div
            ref={list}
            className="console custom-scrollbar h-64 overflow-auto p-3"
            onScroll={(event) => {
              const element = event.currentTarget;
              setFollow(element.scrollHeight - element.scrollTop - element.clientHeight < 24);
            }}
          >
            {game.logs.length === 0 ? (
              <p style={{ color: "var(--text-placeholder)" }}>{t("home.consoleHint")}</p>
            ) : (
              game.logs.map((line) => (
                <div
                  key={line.id}
                  className={`whitespace-pre-wrap break-all ${line.stream === "stderr" ? "line-stderr" : /\b(WARN|ERROR|Exception)\b/.test(line.text) ? "line-warn" : ""}`}
                >
                  {line.stream === "launcher" ? <span style={{ color: "#34d399" }}>[launcher] </span> : null}
                  {line.text}
                </div>
              ))
            )}
          </div>
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="xs" icon="folder_open" onClick={() => void openFolder("gameLogs")}>
              {t("settings.logs.openGame")}
            </Button>
            <span className="flex-1" />
            <IconButton icon="delete_sweep" label="Vider" size={16} onClick={clearLogs} />
          </div>
        </div>
      ) : null}
    </Card>
  );
}
