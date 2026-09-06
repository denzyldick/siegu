import React from 'react';
import { Composition } from 'remotion';
import { Intro } from './Intro';

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="SieguIntro"
        component={Intro}
        durationInFrames={520}
        fps={30}
        width={1920}
        height={1080}
      />
    </>
  );
};
