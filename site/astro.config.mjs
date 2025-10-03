import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
  site: 'https://runeblaze.github.io',
  base: '/syncx/',
  legacy: {
    collections: false,
  },
  integrations: [
    starlight({
      title: 'syncx Documentation',
      description: 'Rust-backed concurrency primitives for Python.',
      sidebar: [
        {
          label: 'Docs',
          items: ['index'],
        },
      ],
      social: [
        {
          label: 'GitHub',
          href: 'https://github.com/RuneBlaze/syncx',
          icon: 'github',
        },
      ],
      editLink: {
        baseUrl: 'https://github.com/RuneBlaze/syncx/tree/main/site',
      },
    }),
  ],
});
