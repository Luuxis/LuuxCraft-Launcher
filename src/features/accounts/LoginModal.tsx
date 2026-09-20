/**
 * Sign-in dialog. Methods come from the panel (`auth_methods_get`), so adding
 * or disabling one there changes this dialog without code changes.
 *
 * Microsoft uses the dedicated login window (authorization code flow).
 */
import { useEffect, useState, type FormEvent } from "react";

import { describeError, t } from "../../i18n";
import { ipc, toAppError } from "../../lib/ipc";
import type { AccountSummary, AppError, AuthMethods } from "../../lib/types";
import { useActions } from "../../store/AppStore";
import { Button } from "../../components/ui/Button";
import { Field, Input, PasswordInput } from "../../components/ui/forms";
import { Icon } from "../../components/ui/Icon";
import { Modal } from "../../components/ui/Modal";
import { Notice, Skeleton } from "../../components/ui/primitives";

type Method = "microsoft" | "azauth" | "offline" | "yggdrasil";

interface LoginModalProps {
  open: boolean;
  onClose: () => void;
  /** Rendered inside a frame placed by the theme, without our own dialog chrome. */
  inline?: boolean;
  title?: string;
  subtitle?: string;
}

export function LoginModal({ open, onClose, inline = false, title, subtitle }: LoginModalProps) {
  const { accountAdded } = useActions();
  const [methods, setMethods] = useState<AuthMethods | null>(null);
  const [methodsError, setMethodsError] = useState<AppError | null>(null);
  const [method, setMethod] = useState<Method | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<AppError | null>(null);
  const [windowOpen, setWindowOpen] = useState(false);
  const [otpRequired, setOtpRequired] = useState(false);

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [code, setCode] = useState("");
  const [username, setUsername] = useState("");

  useEffect(() => {
    if (!open) return;
    setMethods(null);
    setMethodsError(null);
    setMethod(null);
    setError(null);
    setWindowOpen(false);
    setOtpRequired(false);
    setPassword("");
    setCode("");
    ipc
      .authMethods()
      .then((available) => {
        setMethods(available);
        const first: Method | null = available.microsoft
          ? "microsoft"
          : available.azauth
            ? "azauth"
            : available.offline
              ? "offline"
              : available.yggdrasil
                ? "yggdrasil"
                : null;
        setMethod(first);
      })
      .catch((cause) => setMethodsError(toAppError(cause)));
  }, [open]);

  const finish = async (account: AccountSummary) => {
    await accountAdded(account);
    onClose();
  };

  const close = () => {
    if (pending && method === "microsoft") void ipc.authCancel();
    onClose();
  };

  const startMicrosoft = async () => {
    setPending(true);
    setError(null);
    try {
      const account = await ipc.authMicrosoftWindow((event) => {
        if (event.event === "windowOpened") setWindowOpen(true);
      });
      await finish(account);
    } catch (cause) {
      const appError = toAppError(cause);
      if (appError.code !== "cancelled") setError(appError);
    } finally {
      setPending(false);
      setWindowOpen(false);
    }
  };

  const cancelMicrosoft = async () => {
    await ipc.authCancel().catch(() => undefined);
  };

  const submitAzAuth = async (event: FormEvent) => {
    event.preventDefault();
    setPending(true);
    setError(null);
    try {
      const outcome = await ipc.authAzAuth(email, password, otpRequired ? code : undefined);
      if (outcome.status === "otpRequired") {
        setOtpRequired(true);
      } else {
        await finish(outcome.account);
      }
    } catch (cause) {
      setError(toAppError(cause));
    } finally {
      setPending(false);
    }
  };

  const submitOffline = async (event: FormEvent) => {
    event.preventDefault();
    setPending(true);
    setError(null);
    try {
      await finish(await ipc.authOffline(username));
    } catch (cause) {
      setError(toAppError(cause));
    } finally {
      setPending(false);
    }
  };

  const submitYggdrasil = async (event: FormEvent) => {
    event.preventDefault();
    setPending(true);
    setError(null);
    try {
      await finish(await ipc.authYggdrasil(username, password));
    } catch (cause) {
      setError(toAppError(cause));
    } finally {
      setPending(false);
    }
  };

  const tabs: { id: Method; label: string; icon: string; available: boolean }[] = [
    { id: "microsoft", label: t("login.microsoft"), icon: "verified_user", available: methods?.microsoft ?? false },
    { id: "azauth", label: t("login.azauth"), icon: "language", available: Boolean(methods?.azauth) },
    { id: "offline", label: t("login.offline"), icon: "person", available: methods?.offline ?? false },
    { id: "yggdrasil", label: t("login.yggdrasil"), icon: "key", available: Boolean(methods?.yggdrasil) },
  ];
  const availableTabs = tabs.filter((tab) => tab.available);

  return (
    <Modal
      open={open}
      onClose={close}
      inline={inline}
      title={title ?? t("login.title")}
      subtitle={subtitle ?? t("login.subtitle")}
      icon="login"
      locked={pending && method !== "microsoft"}
    >
      {methodsError ? <Notice tone="error">{describeError(methodsError)}</Notice> : null}
      {!methods && !methodsError ? (
        <div className="space-y-3">
          <Skeleton className="h-12" />
          <Skeleton className="h-12" />
        </div>
      ) : null}
      {methods && availableTabs.length === 0 ? <Notice tone="warning">{t("login.noMethod")}</Notice> : null}

      {availableTabs.length > 1 ? (
        <div className="grid gap-2" style={{ gridTemplateColumns: `repeat(${Math.min(availableTabs.length, 4)}, minmax(0, 1fr))` }} role="tablist">
          {availableTabs.map((tab) => (
            <button
              key={tab.id}
              type="button"
              role="tab"
              aria-selected={method === tab.id}
              className="option-card flex-col items-center text-center gap-2 py-3"
              aria-checked={method === tab.id}
              disabled={pending}
              onClick={() => {
                setMethod(tab.id);
                setError(null);
                setOtpRequired(false);
              }}
            >
              <Icon name={tab.icon} size={22} style={{ color: method === tab.id ? "var(--accent-400)" : "var(--text-meta)" }} />
              <span className="text-xs font-semibold" style={{ color: "var(--text-primary)" }}>
                {tab.label}
              </span>
            </button>
          ))}
        </div>
      ) : null}

      {method === "microsoft" ? (
        <div className="space-y-4">
          <p className="text-sm leading-relaxed" style={{ color: "var(--text-body)" }}>
            {t("login.microsoftHint")}
          </p>
          {windowOpen ? (
            <div className="card-inset p-5 space-y-3 text-center">
              <p className="text-sm font-semibold inline-flex items-center gap-2" style={{ color: "var(--text-primary)" }}>
                <Icon name="autorenew" size={16} spin /> {t("login.microsoftWindowOpen")}
              </p>
              <p className="text-xs" style={{ color: "var(--text-meta)" }}>
                {t("login.microsoftWindowHint")}
              </p>
            </div>
          ) : null}
          {error ? <Notice tone="error" details={error.details}>{describeError(error)}</Notice> : null}
          <div className="flex flex-wrap items-center justify-end gap-3">
            {pending ? (
              <Button variant="ghost" onClick={cancelMicrosoft}>
                {t("common.cancel")}
              </Button>
            ) : null}
            <Button variant="primary" size="lg" icon="login" loading={pending} onClick={startMicrosoft}>
              {pending ? t("login.connecting") : t("login.microsoftWindow")}
            </Button>
          </div>
        </div>
      ) : null}

      {method === "azauth" ? (
        <form className="space-y-4" onSubmit={submitAzAuth}>
          {otpRequired ? (
            <>
              <Notice tone="info" icon="security" title={t("login.otpTitle")}>
                {t("login.otpText")}
              </Notice>
              <Field label={t("login.otp")} icon="pin" htmlFor="login-otp">
                <Input id="login-otp" value={code} onChange={(e) => setCode(e.target.value)} inputMode="numeric" autoComplete="one-time-code" autoFocus mono maxLength={12} />
              </Field>
            </>
          ) : (
            <>
              <Field label={t("login.email")} icon="person" htmlFor="login-email">
                <Input id="login-email" value={email} onChange={(e) => setEmail(e.target.value)} autoComplete="username" autoFocus />
              </Field>
              <Field label={t("login.password")} icon="lock" htmlFor="login-password">
                <PasswordInput id="login-password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="current-password" />
              </Field>
            </>
          )}
          {error ? <Notice tone="error" details={error.details}>{describeError(error)}</Notice> : null}
          <div className="flex justify-end gap-3">
            {otpRequired ? (
              <Button variant="ghost" onClick={() => setOtpRequired(false)} disabled={pending}>
                {t("common.back")}
              </Button>
            ) : null}
            <Button type="submit" variant="primary" size="lg" icon="login" loading={pending} disabled={otpRequired ? code.trim().length < 4 : !email || !password}>
              {pending ? t("login.connecting") : t("login.submit")}
            </Button>
          </div>
        </form>
      ) : null}

      {method === "offline" ? (
        <form className="space-y-4" onSubmit={submitOffline}>
          <Field label={t("login.username")} icon="person" htmlFor="login-username" hint={t("login.usernameHint")}>
            <Input id="login-username" value={username} onChange={(e) => setUsername(e.target.value)} autoFocus maxLength={16} />
          </Field>
          {error ? <Notice tone="error" details={error.details}>{describeError(error)}</Notice> : null}
          <div className="flex justify-end">
            <Button type="submit" variant="primary" size="lg" icon="login" loading={pending} disabled={username.trim().length < 3}>
              {t("login.submit")}
            </Button>
          </div>
        </form>
      ) : null}

      {method === "yggdrasil" ? (
        <form className="space-y-4" onSubmit={submitYggdrasil}>
          <p className="text-xs font-mono" style={{ color: "var(--text-meta)" }}>
            {methods?.yggdrasil}
          </p>
          <Field label={t("login.username")} icon="person" htmlFor="login-ygg-username">
            <Input id="login-ygg-username" value={username} onChange={(e) => setUsername(e.target.value)} autoFocus />
          </Field>
          <Field label={t("login.password")} icon="lock" htmlFor="login-ygg-password">
            <PasswordInput id="login-ygg-password" value={password} onChange={(e) => setPassword(e.target.value)} />
          </Field>
          {error ? <Notice tone="error" details={error.details}>{describeError(error)}</Notice> : null}
          <div className="flex justify-end">
            <Button type="submit" variant="primary" size="lg" icon="login" loading={pending} disabled={!username || !password}>
              {t("login.submit")}
            </Button>
          </div>
        </form>
      ) : null}
    </Modal>
  );
}
