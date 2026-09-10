import { useMemo, useState, type MouseEvent } from "react";

import { t } from "../../i18n";
import { formatDate, textPreview } from "../../lib/format";
import { sanitizeHtml } from "../../lib/sanitize";
import type { Article } from "../../lib/types";
import { useActions, useAppState } from "../../store/AppStore";
import { Button, IconButton } from "../../components/ui/Button";
import { Icon } from "../../components/ui/Icon";
import { Card, EmptyState, IconChip, Skeleton } from "../../components/ui/primitives";

export function NewsFeed() {
  const { remote, remoteLoading } = useAppState();
  const { refreshRemote } = useActions();
  const articles = remote?.articles ?? [];

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <IconChip icon="article" size="sm" tone="pink" />
          <h2 className="text-xl font-bold tracking-tight" style={{ color: "var(--text-primary)" }}>
            {t("home.news")}
          </h2>
        </div>
        <IconButton icon="refresh" label={t("common.refresh")} loading={remoteLoading} onClick={() => void refreshRemote()} size={16} />
      </div>
      {remoteLoading && articles.length === 0 ? (
        <div className="space-y-3">
          <Skeleton className="h-28" />
          <Skeleton className="h-28" />
        </div>
      ) : articles.length === 0 ? (
        <Card static>
          <EmptyState icon="inbox" title={t("home.newsEmpty")} description={t("home.newsEmptyHint")} compact />
        </Card>
      ) : (
        <ul className="space-y-3">
          {articles.map((article, index) => (
            <li key={article.id} className={`animate-fade-in-up stagger-${Math.min(index + 1, 5)}`} style={{ opacity: 0 }}>
              <ArticleCard article={article} />
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function ArticleCard({ article }: { article: Article }) {
  const { openExternal } = useActions();
  const [expanded, setExpanded] = useState(false);
  const html = useMemo(() => sanitizeHtml(article.content), [article.content]);
  const preview = useMemo(() => textPreview(article.content, 220), [article.content]);
  const date = formatDate(article.publishedAt);
  const long = preview.length >= 220 || /<(img|h[1-6]|ul|ol|table)/i.test(article.content);

  const onContentClick = (event: MouseEvent<HTMLDivElement>) => {
    const anchor = (event.target as HTMLElement).closest("a");
    if (anchor?.getAttribute("href")) {
      event.preventDefault();
      void openExternal(anchor.getAttribute("href")!);
    }
  };

  return (
    <Card className="overflow-hidden" padding="p-0">
      {article.image ? (
        <div className="h-36 overflow-hidden" style={{ borderBottom: "1px solid color-mix(in srgb, var(--border) 60%, transparent)" }}>
          <img src={article.image} alt="" className="w-full h-full object-cover" loading="lazy" />
        </div>
      ) : null}
      <div className="flex items-center gap-3 px-4 py-3.5" style={{ borderBottom: "1px solid color-mix(in srgb, var(--border) 60%, transparent)" }}>
        <div className="flex-1 min-w-0">
          <h3 className="text-[15px] font-bold truncate" style={{ color: "var(--text-primary)" }}>
            {article.title}
          </h3>
          <p className="text-[11px] mt-0.5 flex items-center gap-2" style={{ color: "var(--text-meta)" }}>
            {article.author ? (
              <span className="inline-flex items-center gap-1">
                <Icon name="person" size={12} /> {article.author}
              </span>
            ) : null}
            {date ? (
              <span className="inline-flex items-center gap-1">
                <Icon name="calendar_month" size={12} /> {date}
              </span>
            ) : null}
          </p>
        </div>
        {article.url ? <IconButton icon="open_in_new" label={t("common.open")} size={16} onClick={() => void openExternal(article.url!)} /> : null}
      </div>
      <div className="px-4 py-3">
        {expanded ? (
          <div className="rich-content" dangerouslySetInnerHTML={{ __html: html }} onClick={onContentClick} />
        ) : (
          <p className="rich-content">{preview || "—"}</p>
        )}
        {long ? (
          <div className="mt-2">
            <Button variant="ghost" size="xs" iconRight={expanded ? "expand_less" : "expand_more"} onClick={() => setExpanded((e) => !e)}>
              {expanded ? t("home.readLess") : t("home.readMore")}
            </Button>
          </div>
        ) : null}
      </div>
    </Card>
  );
}
