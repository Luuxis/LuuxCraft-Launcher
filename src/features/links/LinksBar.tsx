import { t } from "../../i18n";
import { presentLink } from "../../lib/links";
import type { Link } from "../../lib/types";
import { useActions, useAppState } from "../../store/AppStore";
import { BrandIcon, Icon } from "../../components/ui/Icon";
import { useTooltip } from "../../components/ui/Tooltip";

/** Links published by the panel (or the launcher fallback), fully dynamic. */
export function LinksBar({ compact = false }: { compact?: boolean }) {
  const { remote } = useAppState();
  const links = remote?.links ?? [];
  if (links.length === 0) return null;
  return (
    <div className={compact ? "flex flex-wrap gap-2" : "space-y-2"}>
      {!compact ? <span className="group-label px-1">{t("nav.links")}</span> : null}
      <ul className="flex flex-wrap gap-2">
        {links.map((link) => (
          <LinkButton key={`${link.url}-${link.label}`} link={link} />
        ))}
      </ul>
    </div>
  );
}

function LinkButton({ link }: { link: Link }) {
  const { openExternal } = useActions();
  const presentation = presentLink(link);
  const { triggerProps, node } = useTooltip(link.label);
  return (
    <li>
      <button
        type="button"
        className="icon-button w-11 h-11 rounded-xl"
        aria-label={link.label}
        onClick={() => void openExternal(link.url)}
        style={{ borderRadius: 12 }}
        {...triggerProps}
      >
        {presentation.brandPath ? <BrandIcon path={presentation.brandPath} size={18} /> : <Icon name={presentation.symbol} size={20} />}
      </button>
      {node}
    </li>
  );
}
