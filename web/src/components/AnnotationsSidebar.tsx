import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import {
  listAnnotations, createAnnotation, updateAnnotation, deleteAnnotation,
  type Annotation,
} from '../api/annotations';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';

interface Props { 
  entryId: string; 
  compact?: boolean; 
  preselectedQuote?: string;
}

export default function AnnotationsSidebar({ entryId, compact, preselectedQuote }: Props) {
  const [newQuote, setNewQuote] = useState(preselectedQuote ?? '');
  const [newText, setNewText] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editText, setEditText] = useState('');
  const qc = useQueryClient();

  const { data: annotations = [] } = useQuery({
    queryKey: ['annotations', entryId],
    queryFn: () => listAnnotations(entryId),
  });

  const create = useMutation({
    mutationFn: () => createAnnotation(entryId, newQuote, newText),
    onSuccess: () => { setNewQuote(''); setNewText(''); qc.invalidateQueries({ queryKey: ['annotations', entryId] }); },
    onError: () => toast.error('添加批注失败'),
  });

  const update = useMutation({
    mutationFn: ({ id, text }: { id: string; text: string }) => updateAnnotation(id, text),
    onSuccess: () => { setEditingId(null); qc.invalidateQueries({ queryKey: ['annotations', entryId] }); },
    onError: () => toast.error('更新批注失败'),
  });

  const remove = useMutation({
    mutationFn: (id: string) => deleteAnnotation(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['annotations', entryId] }),
    onError: () => toast.error('删除批注失败'),
  });

  return (
    <div className={compact ? 'bg-card p-4 overflow-y-auto' : 'w-80 border-l border-border bg-card p-4 overflow-y-auto'}>
      <h3 className="font-medium text-card-foreground mb-4">批注</h3>

      <div className="mb-4 p-3 bg-secondary rounded-lg border border-border">
        {newQuote && (
          <blockquote className="text-sm text-muted-foreground border-l-2 border-accent pl-2 mb-2 italic">
            {newQuote}
          </blockquote>
        )}
        <textarea
          value={newText}
          onChange={(e) => setNewText(e.target.value)}
          placeholder={newQuote ? '添加笔记...' : '选中文章中的文字以添加批注'}
          className="w-full text-sm px-2 py-1 border border-border rounded-md resize-none h-16 bg-background text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
        />
        <Button 
          onClick={() => create.mutate()} 
          disabled={!newQuote || create.isPending} 
          size="sm" 
          className="mt-1"
        >
          添加
        </Button>
      </div>

      {annotations.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          {newQuote ? '添加笔记后点击「添加」保存批注。' : '选中文章中的文字即可添加批注。'}
        </p>
      ) : (
        <div className="space-y-3">
          {annotations.map((ann: Annotation) => (
            <div key={ann.id} className={`p-3 bg-secondary rounded-lg border ${ann.is_orphaned ? 'border-warning/50' : 'border-border'}`}>
              {ann.is_orphaned && <span className="text-xs text-warning block mb-1">已失效（内容被编辑）</span>}
              <blockquote className="text-sm text-muted-foreground border-l-2 border-muted-foreground/30 pl-2 mb-2 italic">{ann.quote}</blockquote>
              {editingId === ann.id ? (
                <div>
                  <textarea value={editText} onChange={(e) => setEditText(e.target.value)}
                    className="w-full text-sm px-2 py-1 border border-border rounded-md resize-none h-12 bg-background text-foreground" />
                  <div className="flex gap-1 mt-1">
                    <Button size="sm" onClick={() => update.mutate({ id: ann.id, text: editText })}>保存</Button>
                    <Button size="sm" variant="ghost" onClick={() => setEditingId(null)}>取消</Button>
                  </div>
                </div>
              ) : (
                <>
                  {ann.text && <p className="text-sm mb-2 text-card-foreground">{ann.text}</p>}
                  <div className="flex gap-2">
                    <Button size="sm" variant="ghost" onClick={() => { setEditingId(ann.id); setEditText(ann.text); }}>
                      编辑
                    </Button>
                    <Button size="sm" variant="ghost" className="text-destructive hover:text-destructive" onClick={() => remove.mutate(ann.id)}>
                      删除
                    </Button>
                  </div>
                </>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
