import express from "express";
import path from "path";
import formidable from "formidable";
import ffmpegPath from "ffmpeg-static";
import child_process from "child_process";

const app = express();

app.get("/", (req, res) => {
  res.sendFile("src/index.html", { root: path.dirname(".") });
});

app.post("/upload", (req, res) => {
  console.log(req.body);
  const incomingForm = new formidable.IncomingForm();
  incomingForm.parse(req).on("file", (_, file) => {
    console.log(file.filepath, file.originalFilename);
    child_process.exec(
      `${ffmpegPath} -i ${file.filepath} -b:v 5K -y ${path.resolve(
        path.dirname("."),
        "output.mp4"
      )}`,
      () => {
        console.log("returning");
        res.sendFile("output.mp4", { root: path.dirname(".") });
      }
    );
  });
});

if (process.env.MODE === "prod") {
  app.listen(443);
} else {
  console.log("Listening on http://localhost:3000");
  app.listen(3000);
}
