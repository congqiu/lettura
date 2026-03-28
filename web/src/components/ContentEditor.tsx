import { useEditor, EditorContent } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import Highlight from '@tiptap/extension-highlight';
import Underline from '@tiptap/extension-underline';

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
    <div className="border border-gray-200 dark:border-gray-700 rounded-lg overflow-hidden">
      <div className="flex items-center gap-1 p-2 bg-gray-50 dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700 flex-wrap">
        <ToolBtn active={editor.isActive('bold')} onClick={() => editor.chain().focus().toggleBold().run()} label="B" className="font-bold" />
        <ToolBtn active={editor.isActive('italic')} onClick={() => editor.chain().focus().toggleItalic().run()} label="I" className="italic" />
        <ToolBtn active={editor.isActive('underline')} onClick={() => editor.chain().focus().toggleUnderline().run()} label="U" className="underline" />
        <ToolBtn active={editor.isActive('strike')} onClick={() => editor.chain().focus().toggleStrike().run()} label="S" className="line-through" />
        <span className="w-px h-5 bg-gray-300 dark:bg-gray-600 mx-1" />
        <ToolBtn active={editor.isActive('heading', { level: 2 })} onClick={() => editor.chain().focus().toggleHeading({ level: 2 }).run()} label="H2" />
        <ToolBtn active={editor.isActive('heading', { level: 3 })} onClick={() => editor.chain().focus().toggleHeading({ level: 3 }).run()} label="H3" />
        <span className="w-px h-5 bg-gray-300 dark:bg-gray-600 mx-1" />
        <ToolBtn active={editor.isActive('bulletList')} onClick={() => editor.chain().focus().toggleBulletList().run()} label="列表" />
        <ToolBtn active={editor.isActive('blockquote')} onClick={() => editor.chain().focus().toggleBlockquote().run()} label="引用" />
        <ToolBtn active={editor.isActive('codeBlock')} onClick={() => editor.chain().focus().toggleCodeBlock().run()} label="代码" />
        <ToolBtn active={editor.isActive('highlight')} onClick={() => editor.chain().focus().toggleHighlight().run()} label="高亮" />
        <div className="flex-1" />
        <button onClick={onCancel} className="px-3 py-1 text-sm text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700 rounded">取消</button>
        <button onClick={() => onSave(editor.getHTML())} className="px-3 py-1 text-sm bg-blue-600 text-white rounded hover:bg-blue-700">保存</button>
      </div>
      <EditorContent editor={editor} className="prose prose-gray dark:prose-invert max-w-none p-4 min-h-[200px] focus:outline-none bg-white dark:bg-gray-900" />
    </div>
  );
}

function ToolBtn({ active, onClick, label, className = '' }: { active: boolean; onClick: () => void; label: string; className?: string }) {
  return (
    <button onClick={onClick}
      className={`px-2 py-1 text-xs rounded ${className} ${active ? 'bg-blue-100 dark:bg-blue-900/50 text-blue-700 dark:text-blue-300' : 'text-gray-600 dark:text-gray-400 hover:bg-gray-200 dark:hover:bg-gray-700'}`}>
      {label}
    </button>
  );
}
