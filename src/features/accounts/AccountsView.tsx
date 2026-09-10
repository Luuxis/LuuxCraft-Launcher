import { useState } from "react";

import { t } from "../../i18n";
import { formatDateTime } from "../../lib/format";
import type { AccountSummary } from "../../lib/types";
import { useActions, useAppState, useSelectedAccount } from "../../store/AppStore";
import { Button, IconButton } from "../../components/ui/Button";
import { Icon } from "../../components/ui/Icon";
import { ConfirmDialog } from "../../components/ui/Modal";
import { Badge, Card, EmptyState, SectionHeader } from "../../components/ui/primitives";
import { SkinFace } from "./SkinFace";

export function AccountsView() {
  const { accounts, bootstrap } = useAppState();
  const { openLogin, selectAccount, removeAccount, refreshAccount, toast } = useActions();
  const selected = useSelectedAccount();
  const [removing, setRemoving] = useState<AccountSummary | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const onRefresh = async (account: AccountSummary) => {
    setBusy(account.uuid);
    const refreshed = await refreshAccount(account.uuid);
    setBusy(null);
    if (refreshed) toast("success", t("accounts.refreshed"));
  };

  return (
    <div className="space-y-6 animate-fade-in-up">
      <SectionHeader
        icon="manage_accounts"
        title={t("accounts.title")}
        description={t("accounts.subtitle")}
        actions={
          <Button variant="primary" size="lg" icon="person_add" onClick={() => openLogin(true)}>
            {t("accounts.add")}
          </Button>
        }
      />

      {bootstrap ? (
        <p className="text-xs flex items-center gap-1.5 px-1" style={{ color: "var(--text-meta)" }}>
          <Icon name="folder" size={14} />
          {t("accounts.storedIn")} <span className="font-mono selectable">{bootstrap.system.accountsFile}</span>
        </p>
      ) : null}

      {accounts.length === 0 ? (
        <Card static>
          <EmptyState
            icon="person_add"
            title={t("accounts.empty")}
            description={t("accounts.emptyHint")}
            action={
              <Button variant="primary" icon="add" square onClick={() => openLogin(true)}>
                {t("accounts.add")}
              </Button>
            }
          />
        </Card>
      ) : (
        <ul className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {accounts.map((account, index) => {
            const isSelected = selected?.uuid === account.uuid;
            return (
              <li key={account.uuid} className={`animate-fade-in-up stagger-${Math.min(index + 1, 5)}`} style={{ opacity: 0 }}>
                <Card
                  premium={isSelected}
                  className={`h-full flex flex-col gap-4 ${isSelected ? "" : ""}`}
                  style={isSelected ? { borderColor: "rgba(34,197,94,0.45)", boxShadow: "var(--glow-sm)" } : undefined}
                >
                  <div className="flex items-start gap-4">
                    <SkinFace account={account} size={56} className="rounded-xl" />
                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 flex-wrap">
                        <h3 className="text-lg font-bold truncate" style={{ color: "var(--text-primary)" }}>
                          {account.name}
                        </h3>
                        {isSelected ? (
                          <Badge variant="brand" dot pulse>
                            {t("accounts.selected")}
                          </Badge>
                        ) : null}
                        {account.needsReauth ? (
                          <Badge variant="gold" icon="warning">
                            {t("accounts.needsReauth")}
                          </Badge>
                        ) : null}
                      </div>
                      <p className="text-xs mt-1 flex flex-wrap items-center gap-x-3 gap-y-1" style={{ color: "var(--text-meta)" }}>
                        <span>{t(`accounts.kinds.${account.kind}`)}</span>
                        {account.gamertag ? <span>Xbox · {account.gamertag}</span> : null}
                        {account.ownership ? <span>{t(`accounts.ownership.${account.ownership}`)}</span> : null}
                        {account.extra?.money !== null && account.extra?.money !== undefined ? <span>{account.extra.money} crédits</span> : null}
                      </p>
                      <p className="text-[11px] font-mono mt-1 truncate selectable" style={{ color: "var(--text-placeholder)" }}>
                        {account.uuid}
                      </p>
                      {account.needsReauth ? (
                        <p className="text-xs mt-2" style={{ color: "#fcd34d" }}>
                          {t("accounts.needsReauthHint")}
                        </p>
                      ) : null}
                    </div>
                  </div>
                  <div className="mt-auto flex items-center gap-2 flex-wrap">
                    {!isSelected ? (
                      <Button variant="primary" size="sm" square icon="check" onClick={() => void selectAccount(account.uuid)}>
                        {t("accounts.select")}
                      </Button>
                    ) : null}
                    {account.needsReauth ? (
                      <Button variant="gold" size="sm" square icon="login" onClick={() => openLogin(true)}>
                        {t("accounts.needsReauth")}
                      </Button>
                    ) : null}
                    <span className="flex-1" />
                    {account.kind !== "offline" ? (
                      <IconButton icon="sync" label={t("accounts.refresh")} loading={busy === account.uuid} onClick={() => void onRefresh(account)} />
                    ) : null}
                    <IconButton icon="delete" label={t("accounts.remove")} danger onClick={() => setRemoving(account)} />
                  </div>
                  {account.expiresAt ? (
                    <p className="text-[10px]" style={{ color: "var(--text-placeholder)" }}>
                      Session · {formatDateTime(Math.floor(account.expiresAt / 1000))}
                    </p>
                  ) : null}
                </Card>
              </li>
            );
          })}
        </ul>
      )}

      <ConfirmDialog
        open={removing !== null}
        title={t("accounts.remove")}
        message={t("accounts.removeConfirm", { name: removing?.name ?? "" })}
        confirmLabel={t("common.delete")}
        danger
        onCancel={() => setRemoving(null)}
        onConfirm={async () => {
          if (removing) await removeAccount(removing.uuid);
          setRemoving(null);
        }}
      />
    </div>
  );
}
