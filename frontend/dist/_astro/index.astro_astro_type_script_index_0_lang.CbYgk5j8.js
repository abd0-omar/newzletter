async function a(){const o=document.getElementById("xkcd-container"),s=document.getElementById("comic-number");if(!(!o||!s))try{o.innerHTML=`
						<div class="flex justify-center items-center min-h-64">
							<span class="loading loading-spinner loading-lg text-primary"></span>
						</div>
					`,s.textContent="Loading...";const t=await fetch("https://corsproxy.io/?https://xkcd.com/info.0.json");if(!t.ok)throw new Error(`Failed to fetch latest comic: ${t.status}`);const n=await t.json();if(!n.num||typeof n.num!="number")throw new Error("Invalid comic data received");const r=Math.floor(Math.random()*n.num)+1,e=await(await fetch(`https://corsproxy.io/?https://xkcd.com/${r}/info.0.json`)).json();s.textContent=`#${e.num}`,o.innerHTML=`
						<h3 class="text-xl font-bold mb-4 text-secondary">
							${e.title}
						</h3>
						
						<div class="mockup-browser bg-base-300 border border-base-300 mb-6">
							<div class="mockup-browser-toolbar">
								<div class="input text-sm">xkcd.com/${e.num}</div>
							</div>
							<div class="bg-base-100 px-6 py-8">
								<img
									src="${e.img}"
									alt="${e.alt}"
									class="mx-auto max-w-full h-auto"
									loading="lazy"
								/>
							</div>
						</div>
						
						<div class="alert alert-info shadow-lg mb-6">
							<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" class="stroke-current shrink-0 w-6 h-6">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
							</svg>
							<div class="text-left">
								<h4 class="font-semibold">Alt Text:</h4>
								<p class="text-sm">${e.alt}</p>
							</div>
						</div>
						
						<div class="card-actions justify-center">
							<div class="join">
								<div class="tooltip" data-tip="Published Date">
									<button class="btn join-item btn-outline btn-sm">
										📅 ${e.day}/${e.month}/${e.year}
									</button>
								</div>
								<button class="btn join-item btn-secondary btn-sm" id="new-random-btn">
									<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" class="w-4 h-4 stroke-current mr-1">
										<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
									</svg>
									New Random
								</button>
								<a
									href="https://xkcd.com/${e.num}"
									target="_blank"
									rel="noopener noreferrer"
									class="btn join-item btn-primary btn-sm"
								>
									<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" class="w-4 h-4 stroke-current">
										<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 6H6a2 2 0 00-2 2v10a2 2 0 002 2h10a2 2 0 002-2v-4M14 4h6m0 0v6m0-6L10 14"></path>
									</svg>
									View on XKCD
								</a>
							</div>
						</div>
					`;const i=document.getElementById("new-random-btn");i&&i.addEventListener("click",a)}catch(t){console.error("Error fetching XKCD comic:",t),o.innerHTML=`
						<div class="alert alert-error">
							<svg xmlns="http://www.w3.org/2000/svg" class="stroke-current shrink-0 w-6 h-6" fill="none" viewBox="0 0 24 24">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" />
							</svg>
							<div>
								<h3 class="font-bold">Failed to load XKCD comic</h3>
								<p class="text-sm">Please check your internet connection and try again.</p>
							</div>
						</div>
						<div class="mt-4 text-center">
							<button class="btn btn-primary" id="try-again-btn">
								<svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" class="w-4 h-4 stroke-current mr-1">
									<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
								</svg>
								Try Again
							</button>
						</div>
					`;const n=document.getElementById("try-again-btn");n&&n.addEventListener("click",a),s.textContent="Error"}}document.addEventListener("DOMContentLoaded",()=>{a();const o=new URLSearchParams(window.location.search);if(o.get("subscribed")==="true"){const t=document.getElementById("subscription-success");t&&(t.classList.remove("hidden"),t.scrollIntoView({behavior:"smooth",block:"center"}),window.history.replaceState({},"","/"))}const s=o.get("error");if(s){const t=document.getElementById("subscription-error"),n=document.getElementById("error-message");if(t&&n){const r={validation:"Invalid name or email. Please check your input.",captcha:"Captcha verification failed. Please try again.",server:"Server error. Please try again later."};n.textContent=r[s]||"Something went wrong. Please try again.",t.classList.remove("hidden"),t.scrollIntoView({behavior:"smooth",block:"center"}),window.history.replaceState({},"","/")}}});
