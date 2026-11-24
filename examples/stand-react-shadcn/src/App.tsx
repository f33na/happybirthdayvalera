import { Moon, Send, Sun } from 'lucide-react';
import { ListenerPill, MicButton, resolvePreferredLang, useDictation, type Lang } from 'happybirthdayvalera';
import 'happybirthdayvalera/style.css';
import { useEffect, useRef, useState, type RefObject } from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Textarea } from '@/components/ui/textarea';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { VoiceListener } from '@/components/voice-listener';
import { model, modelBaseUrl } from './model-base-url';

const initialDark = (): boolean => {
  const mode = new URLSearchParams(location.search).get('mode');
  return mode ? mode === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches;
};

interface ToolbarProps {
  target: RefObject<HTMLTextAreaElement | null>;
  lang: Lang;
  onLang: (l: Lang) => void;
}

/** Path B: our `MicButton asChild` over the app's own shadcn `Button`, with the `shadcn` theme preset. */
function AsChildToolbar({ target, lang, onLang }: ToolbarProps) {
  const listener = useDictation(target, {
    theme: 'shadcn',
    model,
    modelBaseUrl,
    easterEgg: true,
    onResult: (_text, l) => onLang(l),
  });
  return (
    <div className="flex w-full items-center gap-2">
      <MicButton listener={listener} asChild>
        <Button variant="outline" size="icon" className="rounded-full" />
      </MicButton>
      {listener.pillVisible ? <ListenerPill listener={listener} /> : null}
      <Badge variant="secondary" className="ml-auto">
        {(listener.langLocked ? listener.lang : lang).toUpperCase()}
      </Badge>
    </div>
  );
}

export function App() {
  const [dark, setDark] = useState(initialDark);
  const [text, setText] = useState('');
  const [lang, setLang] = useState<Lang>(() => resolvePreferredLang(navigator.languages));
  // Both toolbars dictate straight into this field; it stays a normal controlled Textarea.
  const field = useRef<HTMLTextAreaElement>(null);

  // shadcn's own mechanism: `.dark` on <html>. The widget tokens resolve on the widget element, so they follow.
  useEffect(() => {
    document.documentElement.classList.toggle('dark', dark);
  }, [dark]);

  return (
    <TooltipProvider>
      <main className="mx-auto flex min-h-svh max-w-xl flex-col justify-center gap-4 p-6">
        <Card>
          <CardHeader>
            <CardTitle>Voice dictation</CardTitle>
            <CardDescription>Speak in English, Russian, German or French. The text lands below.</CardDescription>
            <div className="col-start-2 row-span-2 row-start-1 self-start justify-self-end">
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="ghost" size="icon" aria-label="Toggle dark mode" onClick={() => setDark((d) => !d)}>
                    {dark ? <Sun /> : <Moon />}
                  </Button>
                </TooltipTrigger>
                <TooltipContent>{dark ? 'Light mode' : 'Dark mode'}</TooltipContent>
              </Tooltip>
            </div>
          </CardHeader>
          <CardContent>
            <Textarea
              ref={field}
              aria-label="Message"
              placeholder="Your dictation appears here…"
              rows={5}
              value={text}
              onChange={(e) => setText(e.target.value)}
            />
          </CardContent>
          <CardFooter className="flex-col items-stretch gap-3">
            <Tabs defaultValue="item">
              <TabsList>
                <TabsTrigger value="item">Registry item</TabsTrigger>
                <TabsTrigger value="aschild">MicButton asChild</TabsTrigger>
              </TabsList>
              <TabsContent value="item" className="flex items-center gap-2 pt-2">
                <VoiceListener
                  target={field}
                  easterEgg
                  persistLang
                  model={model}
                  modelBaseUrl={modelBaseUrl}
                  onResult={(_t, l) => setLang(l)}
                />
                <Badge variant="secondary" className="ml-auto">
                  {lang.toUpperCase()}
                </Badge>
              </TabsContent>
              <TabsContent value="aschild" className="pt-2">
                <AsChildToolbar target={field} lang={lang} onLang={setLang} />
              </TabsContent>
            </Tabs>
            <div className="flex justify-end">
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button disabled={!text.trim()} onClick={() => setText('')}>
                    <Send /> Send
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Sends the text and clears the box</TooltipContent>
              </Tooltip>
            </div>
          </CardFooter>
        </Card>
      </main>
    </TooltipProvider>
  );
}
