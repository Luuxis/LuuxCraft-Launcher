import { t } from "../../i18n";
import { presentLink } from "../../lib/links";
import { useActions, useAppState } from "../../store/AppStore";
import { BrandIcon, Icon } from "../../components/ui/Icon";

/** Links published by the panel (or the launcher fallback), fully dynamic. */
export function LinksBar({ compact = false }: { compact?: boolean }) {
  const { remote } = useAppState();
  const { openExternal } = useActions();
  const links = remote?.links ?? [];
  if (links.length === 0) return null;
  return (
    <div className={compact ? "flex flex-wrap gap-2" : "space-y-2"}>
      {!compact ? <span className="group-label px-1">{t("nav.links")}</span> : null}
      <ul className="flex flex-wrap gap-2">
        {links.map((link) => {
          const presentation = presentLink(link);
          return (
            <li key={`${link.url}-${link.label}`}>
              <button
                type="button"
                className="tooltip icon-button w-11 h-11 rounded-xl"
                data-tooltip={link.label}
                aria-label={link.label}
                onClick={() => void openExternal(link.url)}
                style={{ borderRadius: 12 }}
              >
                {presentation.brandPath ? <BrandIcon path={presentation.brandPath} size={18} /> : <Icon name={presentation.symbol} size={20} />}
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
