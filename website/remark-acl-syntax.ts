interface MarkdownNode {
  children?: MarkdownNode[];
  lang?: string | null;
  meta?: string | null;
  type?: string;
}

/**
 * Preserve the product-facing ACL label for the registered presentation
 * grammar. This only controls syntax rendering; product code continues to
 * parse and generate ACL through a3s-acl.
 */
export function remarkAclSyntax() {
  return (tree: MarkdownNode) => {
    const visit = (node: MarkdownNode) => {
      if (node.type === 'code' && node.lang?.toLowerCase() === 'acl') {
        node.meta = [node.meta, 'displayLanguage=ACL']
          .filter(Boolean)
          .join(' ');
      }
      node.children?.forEach(visit);
    };

    visit(tree);
  };
}
