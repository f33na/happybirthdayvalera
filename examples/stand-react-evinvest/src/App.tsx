import {
  Badge,
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  Textarea,
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@evinvest/uikit';
import { useDictation } from 'happybirthdayvalera';
import 'happybirthdayvalera/style.css';
import { EvListenerPill, EvMicButton } from 'happybirthdayvalera/evinvest';
import { useRef, useState } from 'react';
import { model, modelBaseUrl } from './model-base-url';

const initialLight = (): boolean => new URLSearchParams(location.search).get('mode') === 'light';

export function App() {
  // The kit's own mechanism: dark is the default on :root, and `.light` on a wrapper switches the polarity.
  const [light, setLight] = useState(initialLight);
  const [text, setText] = useState('');
  const field = useRef<HTMLTextAreaElement>(null);
  // Dictation goes straight into the field, at the caret; the field stays a normal controlled Textarea.
  const listener = useDictation(field, { theme: 'evinvest', model, modelBaseUrl, easterEgg: true, persistLang: true });

  return (
    <TooltipProvider>
      <div className={light ? 'light' : undefined}>
        <main className="bg-background text-ink flex min-h-svh items-center justify-center p-6">
          <Card className="w-full max-w-xl">
            <CardHeader>
              <div className="flex items-start justify-between gap-4">
                <div className="flex flex-col gap-1">
                  <CardTitle>Voice dictation</CardTitle>
                  <CardDescription>Speak in English, Russian, German or French. The text lands below.</CardDescription>
                </div>
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="md"
                      icon
                      aria-label="Toggle light mode"
                      aria-pressed={light}
                      onClick={() => setLight((v) => !v)}
                    >
                      <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true">
                        <circle cx="12" cy="12" r="4" />
                        <path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
                      </svg>
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>{light ? 'Dark mode' : 'Light mode'}</TooltipContent>
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
            <CardFooter className="flex flex-wrap items-center gap-3">
              <EvMicButton listener={listener} variant="secondary" size="lg" />
              {listener.pillVisible ? <EvListenerPill listener={listener} /> : null}
              <Badge tone="neutral" className="ml-auto">
                {listener.lang.toUpperCase()}
              </Badge>
              <Tooltip>
                <TooltipTrigger asChild>
                  <Button variant="primary" disabled={!text.trim()} onClick={() => setText('')}>
                    Send
                  </Button>
                </TooltipTrigger>
                <TooltipContent>Sends the text and clears the box</TooltipContent>
              </Tooltip>
            </CardFooter>
          </Card>
        </main>
      </div>
    </TooltipProvider>
  );
}
