import React from 'react';
import logoLight from '@/assets/relay-logo-light.png';
import logoDark from '@/assets/relay-logo-dark.png';

interface RelayLogoProps {
  className?: string;
}

/**
 * Relay brand logo using the official brand asset PNGs.
 * Switches between light and dark variants based on the document's
 * current dark-mode class (which Tauri's root `<html class="dark">` sets).
 */
export const RelayLogo: React.FC<RelayLogoProps> = ({ className = 'w-6 h-6' }) => {
  const [isDark, setIsDark] = React.useState(
    () => document.documentElement.classList.contains('dark')
  );

  React.useEffect(() => {
    const observer = new MutationObserver(() => {
      setIsDark(document.documentElement.classList.contains('dark'));
    });
    observer.observe(document.documentElement, { attributeFilter: ['class'] });
    return () => observer.disconnect();
  }, []);

  return (
    <img
      src={isDark ? logoDark : logoLight}
      alt="Relay logo"
      className={className}
      draggable={false}
    />
  );
};
