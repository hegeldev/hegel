// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import starlightThemeRapide from 'starlight-theme-rapide';
import vercel from '@astrojs/vercel';

// https://astro.build/config
export default defineConfig({
    site: 'https://hegel.dev',
    trailingSlash: 'never',
	integrations: [
		starlight({
			title: 'Hegel',
			favicon: '/favicon.jpg',
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/hegeldev' }],
			sidebar: [
				{
					label: 'Introduction',
					items: [
						{ slug: 'intro/getting-started' },
					],
				},
				{
					label: 'Libraries',
					items: [
						{ label: 'hegel-rust', link: 'https://github.com/hegeldev/hegel-rust', attrs: { target: '_blank', class: 'external-link' } },
						{ label: 'hegel-core', link: 'https://github.com/hegeldev/hegel-core', attrs: { target: '_blank', class: 'external-link' } },
					],
				},
				// re-enable once we actually have content here!
				//
				// {
				// 	label: 'How-to guides',
				// 	autogenerate: { directory: 'how-to' },
				// },
				{
					label: 'Explanation',
					items: [
						{ slug: 'explanation/how-hegel-works' },
						{ slug: 'explanation/why-hegel' },
					],
				},
				{
					label: 'Reference',
					items: [
						{ slug: 'reference/installation' },
					],
				},
				{
					slug: 'compatibility',
				},
			],
            customCss: ['./src/styles/style.css'],
            plugins: [ starlightThemeRapide() ],
		}),
	],
    adapter: vercel({
        webAnalytics: { enabled: true },
    }),
});
