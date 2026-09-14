import React from 'react';
import styles from './ScrollFadeHint.module.less';

interface ScrollFadeHintProps {
  /** The element whose scrollTop/scrollHeight decides visibility. */
  scrollContainerRef: React.RefObject<HTMLElement | null>;
}

/**
 * Bottom-of-viewport scroll affordance for tall modal bodies: a translucent
 * gradient that sits at the visible bottom of the real scroll container and only
 * shows while content below remains. Hides at the bottom or when there is no
 * overflow. Never intercepts pointer or keyboard interaction.
 */
const ScrollFadeHint: React.FC<ScrollFadeHintProps> = ({ scrollContainerRef }) => {
  const [visible, setVisible] = React.useState(false);

  const recalculate = React.useCallback(() => {
    const element = scrollContainerRef.current;
    if (!element) {
      setVisible(false);
      return;
    }
    const distanceToBottom = element.scrollHeight - element.scrollTop - element.clientHeight;
    setVisible(distanceToBottom > 1);
  }, [scrollContainerRef]);

  React.useEffect(() => {
    const element = scrollContainerRef.current;
    if (!element) {
      return;
    }

    recalculate();

    const resizeObserver = new ResizeObserver(recalculate);
    // Watch both the content box (its height changes with sections/expansion) and
    // the scroll element itself.
    resizeObserver.observe(element);
    if (element.firstElementChild) {
      resizeObserver.observe(element.firstElementChild);
    }
    window.addEventListener('resize', recalculate);
    element.addEventListener('scroll', recalculate, { passive: true });

    return () => {
      resizeObserver.disconnect();
      window.removeEventListener('resize', recalculate);
      element.removeEventListener('scroll', recalculate);
    };
  }, [scrollContainerRef, recalculate]);

  return (
    <div
      className={`${styles.scrollFadeHint} ${visible ? styles.visible : styles.hidden}`}
      aria-hidden="true"
    />
  );
};

export default ScrollFadeHint;
