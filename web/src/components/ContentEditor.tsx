import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Highlight from '@tiptap/extension-highlight';
import Underline from '@tiptap/extension-underline';
import { Button } from '@/components/ui/button';
import { Separator } from '@/components/ui/separator';

interface Props {
  content: string;
  onSave: (html: string) => void;
  onCancel: () => void;
}

export default function ContentEditor({ content, onSave, onCancel }: Props) {
  const editor = useEditor({
    extensions: [StarterKit, Highlight, Underline],
    content,
  });

  if (!editor) return null;

  return (
    <div className="border border-border rounded-lg overflow-hidden">
      <div className="flex items-center gap-1 p-2 bg-muted/30 border-b border-border flex-wrap">
        <ToolBtn active={editor.isActive('bold')} onClick={() => editor.chain().focus().toggleBold().run()} label="B" className="font-bold" />
        <ToolBtn active={editor.isActive('italic')} onClick={() => editor.chain().focus().toggleItalic().run()} label="I" className="italic" />
        <ToolBtn active={editor.isActive('underline')} onClick={() => editor.chain().focus().toggleUnderline().run()} label="U" className="underline" />
        <ToolBtn active={editor.isActive('strike')} onClick={() => editor.chain().focus().toggleStrike().run()} label="S" className="line-through" />
        <Separator orientation="vertical" className="h-6 mx-1" />
        <ToolBtn active={editor.isActive('heading', { level: 2 })} onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()} label="H2" />
        <ToolBtn active={editor.isActive('heading', { level: 3 })} onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()} label="H3" />
        <Separator orientation="vertical" className="h-6 mx-1" />
        <ToolBtn active={editor.isActive('bulletList')} onClick={() => editor.chain().focus().toggleBulletList().run()} label="列表" />
        <ToolBtn active={editor.isActive('blockquote')} onClick={() => editor.chain().focus().toggleBlockquote().run()} label="引用" />
        <ToolBtn active={editor.isActive('codeBlock')} onClick={() => editor.chain().focus().toggleCodeBlock().run()} label="代码" />
        <ToolBtn active={editor.isActive('highlight')} onClick={() => editor.chain().focus().toggleHighlight().run()} label="高亮" />
        <div className="flex-1" />
        <Button variant="ghost" size="sm" onClick={onCancel}>取消</Button>
        <Button size="sm" onClick={() => onSave(editor.getHTML())}>保存</Button>
      </div>
      <EditorContent editor={editor} className="prose prose-gray dark:prose-invert max-w-none p-4 min-h-[200px] focus:outline-none bg-background dark:bg-muted" />
    </div>
  );
}

function ToolBtn({ active, onClick, label, className = '' }: { active: boolean; onClick: () => void; label: string; className?: string }) {
  return (
    <Button variant="ghost" size="icon" className={`h-8 w-8 text-xs rounded ${className} ${active ? 'bg-accent text-accent-foreground' : ''}`}
      onClick={onClick}>
      {label}
    </Button>
  );
}