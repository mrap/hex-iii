import React, { useEffect, useState } from 'react';
import { Navbar } from '../components/Navbar';
import { Terminal } from '../components/Terminal';
import { CookieConsent } from '../components/CookieConsent';
import { useCookieConsent } from '../lib/useCookieConsent';
import { FooterSection } from '../components/sections/FooterSection';
import { HeroV2 } from '../components/sections-v2/HeroV2';
import { RegistryV2 } from '../components/sections-v2/RegistryV2';
import { TourV2 } from '../components/sections-v2/TourV2';
import { CodeV2 } from '../components/sections-v2/CodeV2';
import { ModelV2 } from '../components/sections-v2/ModelV2';
import { GetStartedV2 } from '../components/sections-v2/GetStartedV2';
import { KeySequence } from '../types';

export const HomeV2: React.FC = () => {
  const { consent, accept, reject } = useCookieConsent();
  const [isDarkMode, setIsDarkMode] = useState(true);
  const [showTerminal, setShowTerminal] = useState(false);
  const [isGodMode, setIsGodMode] = useState(false);
  const [showGodModeUnlock, setShowGodModeUnlock] = useState(false);

  useEffect(() => {
    document.body.classList.toggle('dark-mode', isDarkMode);
    document.body.classList.toggle('light-mode', !isDarkMode);
  }, [isDarkMode]);

  const toggleTheme = () => setIsDarkMode((v) => !v);

  const toggleMode = () => {
    window.history.pushState({}, '', '/ai');
    window.location.reload();
  };

  useEffect(() => {
    let keyBuffer: string[] = [];

    const handleKeyDown = (e: KeyboardEvent) => {
      const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
      keyBuffer.push(key);
      if (keyBuffer.length > 30) keyBuffer = keyBuffer.slice(-30);

      const recentKeys = keyBuffer.slice(-3).join('');
      if (recentKeys === KeySequence.III) setShowTerminal(true);

      const fullHistory = keyBuffer.join('');
      if (fullHistory.includes(KeySequence.KONAMI)) {
        setIsGodMode(true);
        setShowGodModeUnlock(true);
        setTimeout(() => {
          setShowGodModeUnlock(false);
          setShowTerminal(true);
        }, 2000);
        keyBuffer = [];
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  return (
    <div
      className={`min-h-screen font-mono selection:bg-iii-accent selection:text-iii-black relative flex flex-col transition-colors duration-300 ${
        isDarkMode
          ? 'bg-iii-black text-iii-light'
          : 'bg-iii-light text-iii-black'
      } ${isGodMode ? 'selection:bg-red-500' : ''}`}
    >
      <Navbar
        isDarkMode={isDarkMode}
        isGodMode={isGodMode}
        isHumanMode={true}
        onToggleTheme={toggleTheme}
        onToggleMode={toggleMode}
      />

      <main className="flex-1 relative z-10 flex flex-col w-full pt-16 md:pt-20">
        <HeroV2 isDarkMode={isDarkMode} />
        <RegistryV2 isDarkMode={isDarkMode} />
        <TourV2 isDarkMode={isDarkMode} />
        <CodeV2 isDarkMode={isDarkMode} />
        <ModelV2 isDarkMode={isDarkMode} />
        <GetStartedV2 isDarkMode={isDarkMode} />
        <FooterSection isDarkMode={isDarkMode} />
      </main>

      {showGodModeUnlock && (
        <div className="fixed inset-0 z-50 bg-black flex items-center justify-center">
          <div className="text-center animate-pulse">
            <div className="text-6xl md:text-8xl font-black text-red-500 mb-4 tracking-tighter animate-bounce">
              GOD MODE
            </div>
            <div className="text-xl md:text-2xl text-red-400 font-mono tracking-widest">
              UNLOCKED
            </div>
            <div className="mt-8 text-sm text-red-500/50 font-mono">
              ↑↑↓↓←→←→BA
            </div>
          </div>
        </div>
      )}

      {showTerminal && (
        <Terminal
          onClose={() => setShowTerminal(false)}
          isGodMode={isGodMode}
        />
      )}

      {consent === null && (
        <CookieConsent onAccept={accept} onReject={reject} />
      )}
    </div>
  );
};
