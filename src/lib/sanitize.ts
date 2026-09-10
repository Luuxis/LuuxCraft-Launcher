/**
 * Allow-list sanitizer for the HTML produced by the panel's article editor.
 * Runs before anything is injected into the DOM; the window CSP forbids
 * inline scripts anyway, this is the second line of defence.
 */

const ALLOWED_TAGS = new Set([
  "p",
  "br",
  "strong",
  "b",
  "em",
  "i",
  "u",
  "s",
  "h1",
  "h2",
  "h3",
  "h4",
  "h5",
  "h6",
  "ul",
  "ol",
  "li",
  "a",
  "img",
  "blockquote",
  "code",
  "pre",
  "span",
  "div",
  "hr",
  "table",
  "thead",
  "tbody",
  "tr",
  "td",
  "th",
  "sub",
  "sup",
]);

const ALLOWED_ATTRIBUTES: Record<string, Set<string>> = {
  a: new Set(["href", "title"]),
  img: new Set(["src", "alt", "title", "width", "height"]),
  td: new Set(["colspan", "rowspan"]),
  th: new Set(["colspan", "rowspan"]),
};

function isSafeUrl(value: string, allowData: boolean): boolean {
  const trimmed = value.trim().toLowerCase();
  if (trimmed.startsWith("https://") || trimmed.startsWith("http://")) return true;
  if (allowData && trimmed.startsWith("data:image/")) return true;
  return false;
}

export function sanitizeHtml(html: string): string {
  if (typeof DOMParser === "undefined") return "";
  const document = new DOMParser().parseFromString(`<body>${html}</body>`, "text/html");
  const walk = (node: Element) => {
    for (const child of Array.from(node.children)) {
      const tag = child.tagName.toLowerCase();
      if (!ALLOWED_TAGS.has(tag)) {
        // Keep the text of unknown elements, drop the element itself.
        const text = document.createTextNode(child.textContent ?? "");
        child.replaceWith(text);
        continue;
      }
      const allowed = ALLOWED_ATTRIBUTES[tag] ?? new Set<string>();
      for (const attribute of Array.from(child.attributes)) {
        const name = attribute.name.toLowerCase();
        if (!allowed.has(name)) {
          child.removeAttribute(attribute.name);
          continue;
        }
        if (name === "href" && !isSafeUrl(attribute.value, false)) child.removeAttribute(attribute.name);
        if (name === "src" && !isSafeUrl(attribute.value, true)) child.removeAttribute(attribute.name);
      }
      if (tag === "a") {
        child.setAttribute("rel", "noopener noreferrer");
        child.setAttribute("data-external", "true");
      }
      walk(child);
    }
  };
  walk(document.body);
  return document.body.innerHTML;
}
