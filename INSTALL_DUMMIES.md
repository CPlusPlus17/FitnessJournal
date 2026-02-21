# The "Dummies" Guide to Installing Fitness Journal Coach

Welcome! You don't need to be a programmer to set up your own AI fitness coach. We've broken down the process into 6 simple, copy-paste steps. By the end, you'll have a fully functioning AI coach analyzing your Garmin data and talking to you on Signal.

---

## 🧭 Phase 1: Gathering What You Need

Before we touch any code, you need three things:

1. **Docker Desktop**: This is a magic box that runs all the complicated code for you.
   - Go to [Docker's website](https://www.docker.com/products/docker-desktop/) and download Docker Desktop for Mac or Windows.
   - Install it just like a normal app, open it, and leave it running in the background (you should see a little whale icon in your menu bar or system tray).
2. **A Free AI Brain**: We use Google's Gemini to write your workouts.
   - Go to [Google AI Studio](https://aistudio.google.com/app/apikey) and sign in with your Google account.
   - Click **"Create API Key"** and copy the long string of random letters and numbers it gives you. Keep this secret!
3. **A Spare Phone Number for the Bot**: Your AI needs a phone number to talk to you on Signal.
   - You can use a cheap prepaid SIM, a landline, or a VoIP number (like Google Voice). It just needs to be able to receive an SMS or voice call to register with Signal.

---

## 📦 Phase 2: Downloading the Code

1. Scroll to the top of this GitHub page.
2. Click the green **"<> Code"** button.
3. Click **"Download ZIP"**.
4. Once it downloads, extract the ZIP file somewhere easy to find, like your **Desktop**. You should now have a folder called `FitnessJournal-main`.

---

## ⚙️ Phase 3: Setting Up Your Settings

Now we need to tell your new app your secrets.

1. Open the `FitnessJournal-main` folder you just extracted.
2. Find the file named `.env.example`.
3. Rename this file to just `.env` (delete the `.example` part).
   - _Note for Mac users_: Files starting with a dot might be hidden. Press `Cmd + Shift + .` to see hidden files.
   - _Note for Windows users_: Make sure it's not actually named `.env.txt`.
4. Open your new `.env` file using a simple text editor (like TextEdit on Mac, or Notepad on Windows).
5. Fill in the blanks:
   - `GEMINI_API_KEY=your_gemini_api_key_here` (Paste the key you got from Google)
   - `SIGNAL_PHONE_NUMBER=+1234567890` (Put your bot's phone number here. Include the `+` and country code!)
   - `API_AUTH_TOKEN=make_up_a_long_password` (Just type a random string of characters here to protect your data. You won't ever need to remember this password).
6. **Save** the file and close it.

---

## 🔗 Phase 4: Linking Your Garmin Account

We need to give the app permission to read your workouts from Garmin.

1. Open a **Terminal** (on Mac, search for "Terminal" in Spotlight) or **Command Prompt/PowerShell** (on Windows).
2. Type `cd ` (the letters "cd" followed by a space).
3. Drag and drop the `FitnessJournal-main` folder from your Desktop into the Terminal window and press **Enter**. This tells the terminal to look inside that folder.
4. Now, copy and paste this exact command into the terminal and press **Enter**:

   ```bash
   docker-compose run --rm fitness-coach --login
   ```
5. It will ask for your Garmin Email. Type it and press Enter.
6. It will ask for your Garmin Password. As you type, nothing will show up on the screen—this is normal security! Type it carefully and press Enter.
7. If you have Two-Factor Authentication enabled on Garmin, it will ask for the 6-digit code.
8. Once it says **"Login successful!"**, you are connected!

---

## 💬 Phase 5: Linking the Signal Bot

Now we connect the bot's phone number to the Signal network.

1. Go back to your `FitnessJournal-main` folder and open the file named `docker-compose.yml` in your text editor (TextEdit/Notepad).
2. Look around line 8 for this section:
   ```yaml
       environment:
         - MODE=json-rpc
         #- MODE=normal
   ```
3. Swap the hashtags! We want to turn **OFF** `json-rpc` and turn **ON** `normal`. Change it so it looks exactly like this:
   ```yaml
       environment:
         #- MODE=json-rpc
         - MODE=normal
   ```
4. **Save** the file.
5. Go back to your Terminal (make sure you are still in the `FitnessJournal-main` folder), paste this command, and press **Enter**:
   ```bash
   docker-compose up -d signal-api
   ```
6. Open your web browser (Chrome/Safari/Edge) and go to this exact address:
   [http://127.0.0.1:8080/v1/qrcodelink?device_name=Fitness-Coach](http://127.0.0.1:8080/v1/qrcodelink?device_name=Fitness-Coach)
7. A QR code will appear on your screen!
8. Open the Signal app on the phone that has your bot's number. Go to **Settings -> Linked Devices -> Add New Device (+)**, and scan the QR code on your computer screen.
9. **IMPORTANT**: Once you've scanned the code and the device is linked, open `docker-compose.yml` again and swap the hashtags back to how they were at the start:
   ```yaml
       environment:
         - MODE=json-rpc
         #- MODE=normal
   ```
10. **Save** the file again.

---

## 🚀 Phase 6: Launching Your Fitness Journal!

You're at the final step!

1. In your Terminal, paste this final magic command and press **Enter**:
   ```bash
   docker-compose up -d --build
   ```
2. Wait a few minutes. Docker is downloading all the necessary pieces and building your app.
3. Once the terminal activity stops and returns to a prompt, you're live!
4. Open your web browser and go to: [http://localhost:3000](http://localhost:3000)
   - You should see your beautiful new Fitness Dashboard!
5. Open your personal Signal app, add your bot's phone number as a contact, and say hello! Try sending the exact message: `/status`

**🎉 Congratulations! You have successfully installed and deployed your own private AI fitness coach!**
